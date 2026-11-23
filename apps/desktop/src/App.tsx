import type { DiscoveredScale, Plu, Scale } from "@openscale/client";
import { Badge, Button, DataTable, useOpenScaleTransport } from "@openscale/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { Database, Moon, Radar, Scale as ScaleIcon, Sun } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

type Tab = "scales" | "catalog";

export function App() {
  const { t, i18n } = useTranslation();
  const transport = useOpenScaleTransport();
  const queryClient = useQueryClient();

  const [activeTab, setActiveTab] = useState<Tab>("scales");
  const [darkMode, setDarkMode] = useState(false);
  const [discovered, setDiscovered] = useState<DiscoveredScale[]>([]);

  const scalesQuery = useQuery({
    queryKey: ["scales"],
    queryFn: () => transport.listScales(),
  });

  const plusQuery = useQuery({
    queryKey: ["plus"],
    queryFn: () => transport.listPlus(),
  });

  const discoverMutation = useMutation({
    mutationFn: () => transport.discoverScales(),
    onSuccess: (items) => setDiscovered(items),
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
    const scale: Scale = {
      id: crypto.randomUUID(),
      group_id: null,
      name: item.model ?? item.ip_address,
      family: item.family,
      ip_address: item.ip_address,
      data_port: item.data_port,
      service_port: item.service_port,
      serial_number: item.serial_number,
      model: item.model,
      firmware_version: item.firmware_version,
      mac_address: item.mac_address,
      status: "online",
      last_seen_at: now,
      meta: {
        store_id: "01900000-0000-7000-8000-000000000001",
        sync_version: 1,
        updated_at: now,
        deleted_at: null,
      },
    };
    await transport.upsertScale(scale);
    await queryClient.invalidateQueries({ queryKey: ["scales"] });
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

      <main className="flex-1 space-y-6 p-6">
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
                <h3 className="text-sm font-medium text-muted-foreground">LAN Discovery</h3>
                <DataTable
                  columns={discoveredColumns}
                  data={discovered}
                  emptyMessage={t("common.noData")}
                />
              </section>
            )}

            <section className="space-y-2">
              <h3 className="text-sm font-medium text-muted-foreground">Registered Scales</h3>
              <DataTable
                columns={scaleColumns}
                data={scalesQuery.data ?? []}
                emptyMessage={t("common.noData")}
              />
            </section>
          </>
        ) : (
          <>
            <div className="flex items-center justify-between">
              <h2 className="text-xl font-semibold">{t("catalog.title")}</h2>
              <Button
                variant="outline"
                onClick={() => void queryClient.invalidateQueries({ queryKey: ["plus"] })}
              >
                {t("common.refresh")}
              </Button>
            </div>
            <DataTable
              columns={pluColumns}
              data={plusQuery.data ?? []}
              emptyMessage={t("common.noData")}
            />
          </>
        )}
      </main>
    </div>
  );
}
