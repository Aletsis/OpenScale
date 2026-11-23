import {
  type DiscoveredScale,
  type Plu,
  type ProductType,
  type Scale,
  type SyncMode,
  applyBulkPriceAdjustment,
  exportPlusToXlsxBuffer,
  parsePlusFromCsv,
  parsePlusFromXlsxBuffer,
  serializePlusToCsv,
} from "@openscale/client";
import { Badge, Button, DataTable, useOpenScaleTransport } from "@openscale/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import {
  Database,
  Download,
  Moon,
  Percent,
  Plus,
  Radar,
  RefreshCw,
  Scale as ScaleIcon,
  Sun,
  Trash2,
  Upload,
} from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

type Tab = "scales" | "catalog";

const DEFAULT_STORE_ID = "01900000-0000-7000-8000-000000000001";

interface PluFormState {
  did: string;
  plu: string;
  description: string;
  price1: string;
  price2: string;
  productType: ProductType;
  ean: string;
  tareGrams: string;
  dateOffset1: string;
  editingId: string | null;
}

const EMPTY_FORM: PluFormState = {
  did: "1",
  plu: "",
  description: "",
  price1: "0.00",
  price2: "0.00",
  productType: "by_weight",
  ean: "",
  tareGrams: "",
  dateOffset1: "0",
  editingId: null,
};

function triggerBlobDownload(data: BlobPart, filename: string, mimeType: string) {
  const blob = new Blob([data], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function App() {
  const { t, i18n } = useTranslation();
  const transport = useOpenScaleTransport();
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const [activeTab, setActiveTab] = useState<Tab>("scales");
  const [darkMode, setDarkMode] = useState(false);
  const [discovered, setDiscovered] = useState<DiscoveredScale[]>([]);
  const [statusBanner, setStatusBanner] = useState<string | null>(null);

  // Catalog state
  const [search, setSearch] = useState("");
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<PluFormState>(EMPTY_FORM);
  const [showBulk, setShowBulk] = useState(false);
  const [bulkMode, setBulkMode] = useState<"percent" | "fixed_cents">("percent");
  const [bulkValue, setBulkValue] = useState("5");

  const scalesQuery = useQuery({
    queryKey: ["scales"],
    queryFn: () => transport.listScales(),
  });

  const plusQuery = useQuery({
    queryKey: ["plus"],
    queryFn: () => transport.listPlus(),
  });

  const filteredPlus = useMemo(() => {
    const items = plusQuery.data ?? [];
    const q = search.trim().toLowerCase();
    if (!q) {
      return items;
    }
    return items.filter(
      (p) =>
        p.description.toLowerCase().includes(q) ||
        String(p.key.plu).includes(q) ||
        (p.ean ?? "").toLowerCase().includes(q),
    );
  }, [plusQuery.data, search]);

  const discoverMutation = useMutation({
    mutationFn: () => transport.discoverScales(),
    onSuccess: (items) => {
      setDiscovered(items);
      setStatusBanner(`LAN Discovery: ${items.length} scale(s) found.`);
    },
    onError: (err) => setStatusBanner(`Error: ${String(err)}`),
  });

  const importFromScaleMutation = useMutation({
    mutationFn: (scaleId: string) => transport.importFromScale(scaleId),
    onSuccess: async (count) => {
      await queryClient.invalidateQueries({ queryKey: ["plus"] });
      setStatusBanner(`Imported ${count} PLU(s) from scale into SQLite catalog.`);
    },
    onError: (err) => setStatusBanner(`Import error: ${String(err)}`),
  });

  const syncScaleMutation = useMutation({
    mutationFn: ({ scaleId, mode }: { scaleId: string; mode: SyncMode }) =>
      transport.syncScale(scaleId, mode),
    onSuccess: (report) => {
      setStatusBanner(
        `Sync (${report.mode}): ${report.sent_plus} sent, ${report.skipped_unchanged} unchanged, ${report.failed_plus} failed.`,
      );
    },
    onError: (err) => setStatusBanner(`Sync error: ${String(err)}`),
  });

  const toggleTheme = () => {
    setDarkMode((prev) => {
      const next = !prev;
      document.documentElement.classList.toggle("dark", next);
      return next;
    });
  };

  const toggleLanguage = () => {
    void i18n.changeLanguage(i18n.language === "en" ? "es" : "en");
  };

  const registerDiscovered = async (item: DiscoveredScale) => {
    const now = new Date().toISOString();
    const existing = (scalesQuery.data ?? []).find(
      (s) => s.ip_address === item.ip_address && s.data_port === item.data_port,
    );
    const scale: Scale = {
      id: existing?.id ?? crypto.randomUUID(),
      group_id: existing?.group_id ?? null,
      name: item.model ?? existing?.name ?? item.ip_address,
      family: item.family,
      ip_address: item.ip_address,
      data_port: item.data_port,
      service_port: item.service_port,
      serial_number: item.serial_number ?? existing?.serial_number ?? null,
      model: item.model ?? existing?.model ?? null,
      firmware_version: item.firmware_version ?? existing?.firmware_version ?? null,
      mac_address: item.mac_address ?? existing?.mac_address ?? null,
      status: "online",
      last_seen_at: now,
      meta: {
        store_id: existing?.meta.store_id ?? DEFAULT_STORE_ID,
        sync_version: (existing?.meta.sync_version ?? 0) + 1,
        updated_at: now,
        deleted_at: null,
      },
    };
    await transport.upsertScale(scale);
    await queryClient.invalidateQueries({ queryKey: ["scales"] });
    setStatusBanner(
      existing
        ? `Scale ${scale.name} (${scale.ip_address}) updated.`
        : `Scale ${scale.name} (${scale.ip_address}) saved.`,
    );
  };

  const openEditForm = (plu: Plu) => {
    setForm({
      did: String(plu.key.did),
      plu: String(plu.key.plu),
      description: plu.description,
      price1: (plu.price_1 / 100).toFixed(2),
      price2: (plu.price_2 / 100).toFixed(2),
      productType: plu.product_type,
      ean: plu.ean ?? "",
      tareGrams: plu.tare_grams !== null ? String(plu.tare_grams) : "",
      dateOffset1: String(plu.date_offset_1),
      editingId: plu.id,
    });
    setShowForm(true);
  };

  const handleSavePlu = async () => {
    const did = Number.parseInt(form.did, 10);
    const pluNum = Number.parseInt(form.plu, 10);
    if (!Number.isFinite(did) || did < 0 || !Number.isFinite(pluNum) || pluNum <= 0) {
      setStatusBanner("Invalid DID or PLU number.");
      return;
    }

    const now = new Date().toISOString();
    const dateOffset1 = Math.max(0, Number.parseInt(form.dateOffset1 || "0", 10) || 0);
    const tareVal = form.tareGrams.trim() ? Number.parseInt(form.tareGrams, 10) : null;

    const record: Plu = {
      id: form.editingId ?? crypto.randomUUID(),
      key: { did, plu: pluNum },
      description: form.description.trim(),
      button_text: null,
      price_1: Math.max(0, Math.round((Number.parseFloat(form.price1) || 0) * 100)),
      price_2: Math.max(0, Math.round((Number.parseFloat(form.price2) || 0) * 100)),
      price_3: 0,
      product_type: form.productType,
      ean: form.ean.trim() || null,
      sku: null,
      group_id: 0,
      barcode_format_id: 1,
      label_format_id: 1,
      tare_id: null,
      tare_grams: tareVal !== null && Number.isFinite(tareVal) ? Math.max(0, tareVal) : null,
      date_offset_1: dateOffset1,
      date_print_control_1: dateOffset1 > 0 ? 1 : 0,
      date_offset_2: 0,
      date_print_control_2: 0,
      tax_rate_id_1: 0,
      tax_rate_id_2: 0,
      text_areas: [],
      delta_hash: "",
      meta: {
        store_id: DEFAULT_STORE_ID,
        sync_version: 1,
        updated_at: now,
        deleted_at: null,
      },
    };

    await transport.upsertPlu(record);
    await queryClient.invalidateQueries({ queryKey: ["plus"] });
    setShowForm(false);
    setForm(EMPTY_FORM);
    setStatusBanner(`PLU ${did}:${pluNum} saved.`);
  };

  const handleDeletePlu = async (plu: Plu) => {
    await transport.deletePlu(plu.key);
    await queryClient.invalidateQueries({ queryKey: ["plus"] });
    setStatusBanner(`PLU ${plu.key.did}:${plu.key.plu} deleted.`);
  };

  const handleBulkPriceApply = async () => {
    const parsedVal = Number.parseFloat(bulkValue);
    if (!Number.isFinite(parsedVal) || filteredPlus.length === 0) {
      return;
    }
    const adjusted = applyBulkPriceAdjustment(filteredPlus, {
      mode: bulkMode,
      value: bulkMode === "percent" ? parsedVal : Math.round(parsedVal * 100),
    });
    for (const item of adjusted) {
      await transport.upsertPlu(item);
    }
    await queryClient.invalidateQueries({ queryKey: ["plus"] });
    setShowBulk(false);
    setStatusBanner(`Updated prices for ${adjusted.length} PLU(s).`);
  };

  const handleImportFile = async (file: File) => {
    const isExcel = /\.xlsx?$/i.test(file.name);
    const parsed = isExcel
      ? parsePlusFromXlsxBuffer(await file.arrayBuffer(), DEFAULT_STORE_ID)
      : parsePlusFromCsv(await file.text(), DEFAULT_STORE_ID);

    for (const item of parsed) {
      await transport.upsertPlu(item);
    }
    await queryClient.invalidateQueries({ queryKey: ["plus"] });
    setStatusBanner(`Imported ${parsed.length} PLU(s) from ${file.name}.`);
  };

  const handleExportCsv = () => {
    const csv = serializePlusToCsv(filteredPlus);
    triggerBlobDownload(csv, "openscale-plus.csv", "text/csv;charset=utf-8");
  };

  const handleExportXlsx = () => {
    const bytes = exportPlusToXlsxBuffer(filteredPlus);
    triggerBlobDownload(
      bytes.buffer as ArrayBuffer,
      "openscale-plus.xlsx",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    );
  };

  const discoveredColumns: ColumnDef<DiscoveredScale>[] = [
    {
      accessorKey: "family",
      header: "Protocol",
      cell: ({ getValue }) => (
        <Badge variant="default">
          {getValue<string>() === "avery_berkel_xs" ? "XS / M-Series" : "XTi / XTRA"}
        </Badge>
      ),
    },
    { accessorKey: "ip_address", header: "IP" },
    { accessorKey: "data_port", header: "Port" },
    { accessorKey: "model", header: "Model", cell: ({ getValue }) => getValue<string>() ?? "—" },
    {
      accessorKey: "firmware_version",
      header: "Firmware",
      cell: ({ getValue }) => getValue<string>() ?? "—",
    },
    {
      id: "actions",
      header: "",
      cell: ({ row }) => (
        <Button size="sm" variant="outline" onClick={() => void registerDiscovered(row.original)}>
          {t("common.save")}
        </Button>
      ),
    },
  ];

  const scaleColumns: ColumnDef<Scale>[] = [
    { accessorKey: "name", header: "Name" },
    { accessorKey: "ip_address", header: "IP" },
    { accessorKey: "model", header: "Model", cell: ({ getValue }) => getValue<string>() ?? "—" },
    {
      accessorKey: "status",
      header: "Status",
      cell: ({ getValue }) => {
        const status = getValue<Scale["status"]>();
        const variant = status === "online" ? "success" : status === "error" ? "danger" : "default";
        return <Badge variant={variant}>{t(`scales.status.${status}`)}</Badge>;
      },
    },
    {
      id: "syncActions",
      header: t("catalog.columns.actions"),
      cell: ({ row }) => (
        <div className="flex flex-wrap gap-1.5">
          <Button
            size="sm"
            variant="outline"
            disabled={importFromScaleMutation.isPending}
            onClick={() => importFromScaleMutation.mutate(row.original.id)}
          >
            {t("scales.importCatalog")}
          </Button>
          <Button
            size="sm"
            disabled={syncScaleMutation.isPending}
            onClick={() => syncScaleMutation.mutate({ scaleId: row.original.id, mode: "delta" })}
          >
            <RefreshCw className="h-3.5 w-3.5" />
            {t("scales.deltaSync")}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={syncScaleMutation.isPending}
            onClick={() => syncScaleMutation.mutate({ scaleId: row.original.id, mode: "full" })}
          >
            {t("scales.fullSync")}
          </Button>
        </div>
      ),
    },
  ];

  const pluColumns: ColumnDef<Plu>[] = [
    { id: "did", header: t("catalog.columns.did"), accessorFn: (row) => row.key.did },
    { id: "plu", header: t("catalog.columns.plu"), accessorFn: (row) => row.key.plu },
    { accessorKey: "description", header: t("catalog.columns.description") },
    {
      id: "price1",
      header: t("catalog.columns.price1"),
      accessorFn: (row) => `$${(row.price_1 / 100).toFixed(2)}`,
    },
    {
      accessorKey: "ean",
      header: t("catalog.columns.ean"),
      cell: ({ getValue }) => getValue<string>() ?? "—",
    },
    {
      id: "actions",
      header: t("catalog.columns.actions"),
      cell: ({ row }) => (
        <div className="flex gap-1.5">
          <Button size="sm" variant="outline" onClick={() => openEditForm(row.original)}>
            {t("common.edit")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void handleDeletePlu(row.original)}
            aria-label={t("common.delete")}
          >
            <Trash2 className="h-3.5 w-3.5 text-destructive" />
          </Button>
        </div>
      ),
    },
  ];

  return (
    <div className="flex min-h-screen bg-background text-foreground">
      <aside className="flex w-60 flex-col border-r border-border bg-card p-4">
        <div className="mb-6">
          <h1 className="text-lg font-bold tracking-tight">{t("app.name")}</h1>
          <p className="text-xs text-muted-foreground">{t("app.tagline")}</p>
        </div>
        <nav className="flex flex-col gap-1">
          <Button
            variant={activeTab === "scales" ? "default" : "ghost"}
            className="justify-start"
            onClick={() => setActiveTab("scales")}
          >
            <ScaleIcon className="h-4 w-4" />
            {t("nav.scales")}
          </Button>
          <Button
            variant={activeTab === "catalog" ? "default" : "ghost"}
            className="justify-start"
            onClick={() => setActiveTab("catalog")}
          >
            <Database className="h-4 w-4" />
            {t("nav.catalog")}
          </Button>
        </nav>
        <div className="mt-auto flex items-center gap-2 pt-4">
          <Button size="sm" variant="outline" onClick={toggleLanguage}>
            {i18n.language.toUpperCase()}
          </Button>
          <Button size="sm" variant="outline" onClick={toggleTheme} aria-label="Toggle theme">
            {darkMode ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </Button>
        </div>
      </aside>

      <main className="flex-1 space-y-5 p-6">
        {statusBanner && (
          <div className="flex items-center justify-between rounded-md border border-border bg-card px-4 py-2 text-sm">
            <span>{statusBanner}</span>
            <Button size="sm" variant="ghost" onClick={() => setStatusBanner(null)}>
              ×
            </Button>
          </div>
        )}

        {activeTab === "scales" ? (
          <>
            <div className="flex items-center justify-between">
              <h2 className="text-xl font-semibold">{t("nav.scales")}</h2>
              <Button
                onClick={() => discoverMutation.mutate()}
                disabled={discoverMutation.isPending}
              >
                <Radar className="h-4 w-4" />
                {discoverMutation.isPending ? t("scales.discovering") : t("scales.discover")}
              </Button>
            </div>

            {discovered.length > 0 && (
              <section className="space-y-2">
                <h3 className="text-sm font-medium text-muted-foreground">
                  {t("scales.lanDiscovery")}
                </h3>
                <DataTable
                  columns={discoveredColumns}
                  data={discovered}
                  emptyMessage={t("common.noData")}
                />
              </section>
            )}

            <section className="space-y-2">
              <h3 className="text-sm font-medium text-muted-foreground">
                {t("scales.registered")}
              </h3>
              <DataTable
                columns={scaleColumns}
                data={scalesQuery.data ?? []}
                emptyMessage={t("common.noData")}
              />
            </section>
          </>
        ) : (
          <>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h2 className="text-xl font-semibold">{t("catalog.title")}</h2>
              <div className="flex flex-wrap gap-2">
                <input
                  ref={fileInputRef}
                  type="file"
                  accept=".csv,.xlsx,.xls"
                  className="hidden"
                  onChange={(ev) => {
                    const file = ev.target.files?.[0];
                    if (file) {
                      void handleImportFile(file);
                      ev.target.value = "";
                    }
                  }}
                />
                <Button size="sm" variant="outline" onClick={() => fileInputRef.current?.click()}>
                  <Upload className="h-4 w-4" />
                  {t("catalog.importFile")}
                </Button>
                <Button size="sm" variant="outline" onClick={handleExportCsv}>
                  <Download className="h-4 w-4" />
                  {t("catalog.exportCsv")}
                </Button>
                <Button size="sm" variant="outline" onClick={handleExportXlsx}>
                  <Download className="h-4 w-4" />
                  {t("catalog.exportXlsx")}
                </Button>
                <Button size="sm" variant="secondary" onClick={() => setShowBulk((v) => !v)}>
                  <Percent className="h-4 w-4" />
                  {t("catalog.bulkPrice")}
                </Button>
                <Button
                  size="sm"
                  onClick={() => {
                    setForm(EMPTY_FORM);
                    setShowForm(true);
                  }}
                >
                  <Plus className="h-4 w-4" />
                  {t("catalog.addPlu")}
                </Button>
              </div>
            </div>

            <div className="flex items-center gap-2">
              <input
                type="search"
                value={search}
                onChange={(ev) => setSearch(ev.target.value)}
                placeholder={t("catalog.searchPlaceholder")}
                className="w-full max-w-md rounded-md border border-border bg-card px-3 py-1.5 text-sm"
              />
              <Badge variant="default">{filteredPlus.length} PLUs</Badge>
            </div>

            {showBulk && (
              <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-card p-3 text-sm">
                <select
                  value={bulkMode}
                  onChange={(ev) => setBulkMode(ev.target.value as "percent" | "fixed_cents")}
                  className="rounded-md border border-border bg-background px-2 py-1"
                >
                  <option value="percent">% Percent</option>
                  <option value="fixed_cents">$ Fixed (+/-)</option>
                </select>
                <input
                  type="number"
                  step="0.5"
                  value={bulkValue}
                  onChange={(ev) => setBulkValue(ev.target.value)}
                  className="w-28 rounded-md border border-border bg-background px-2 py-1"
                />
                <Button size="sm" onClick={() => void handleBulkPriceApply()}>
                  {t("common.apply")}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setShowBulk(false)}>
                  {t("common.cancel")}
                </Button>
              </div>
            )}

            {showForm && (
              <div className="grid grid-cols-1 gap-3 rounded-md border border-border bg-card p-4 text-sm md:grid-cols-4">
                <label className="flex flex-col gap-1">
                  <span>{t("catalog.columns.did")}</span>
                  <input
                    type="number"
                    value={form.did}
                    onChange={(ev) => setForm({ ...form, did: ev.target.value })}
                    className="rounded-md border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>{t("catalog.columns.plu")}</span>
                  <input
                    type="number"
                    value={form.plu}
                    onChange={(ev) => setForm({ ...form, plu: ev.target.value })}
                    className="rounded-md border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1 md:col-span-2">
                  <span>{t("catalog.columns.description")}</span>
                  <input
                    type="text"
                    value={form.description}
                    onChange={(ev) => setForm({ ...form, description: ev.target.value })}
                    className="rounded-md border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>{t("catalog.columns.price1")} ($)</span>
                  <input
                    type="number"
                    step="0.01"
                    value={form.price1}
                    onChange={(ev) => setForm({ ...form, price1: ev.target.value })}
                    className="rounded-md border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>{t("catalog.columns.ean")}</span>
                  <input
                    type="text"
                    value={form.ean}
                    onChange={(ev) => setForm({ ...form, ean: ev.target.value })}
                    className="rounded-md border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>Tare (g)</span>
                  <input
                    type="number"
                    value={form.tareGrams}
                    onChange={(ev) => setForm({ ...form, tareGrams: ev.target.value })}
                    className="rounded-md border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>Shelf Life (days)</span>
                  <input
                    type="number"
                    value={form.dateOffset1}
                    onChange={(ev) => setForm({ ...form, dateOffset1: ev.target.value })}
                    className="rounded-md border border-border bg-background px-2 py-1"
                  />
                </label>
                <div className="flex justify-end gap-2 md:col-span-4">
                  <Button size="sm" variant="ghost" onClick={() => setShowForm(false)}>
                    {t("common.cancel")}
                  </Button>
                  <Button size="sm" onClick={() => void handleSavePlu()}>
                    {t("common.save")}
                  </Button>
                </div>
              </div>
            )}

            <DataTable columns={pluColumns} data={filteredPlus} emptyMessage={t("common.noData")} />
          </>
        )}
      </main>
    </div>
  );
}
