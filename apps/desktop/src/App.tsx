import {
  type DiscoveredScale,
  type Plu,
  type Scale,
  type SyncMode,
  applyBulkPriceAdjustment,
  exportPlusToXlsxBuffer,
  parsePlusFromCsv,
  parsePlusFromXlsxBuffer,
  serializePlusToCsv,
} from "@openscale/client";
import {
  Badge,
  Button,
  ConfirmDialog,
  DataTable,
  Dialog,
  useOpenScaleTransport,
  useToast,
} from "@openscale/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import {
  Check,
  Database,
  Download,
  Globe,
  History,
  Moon,
  Pencil,
  Percent,
  Plus,
  Radar,
  RefreshCw,
  Scale as ScaleIcon,
  Search,
  Sun,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { type FormEvent, useCallback, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { PluFormDialog } from "./components/PluFormDialog.js";

type Tab = "scales" | "catalog" | "sync";

interface SyncHistoryEntry {
  id: string;
  timestamp: string;
  operation: "delta" | "full" | "import" | "discover";
  target: string;
  status: "success" | "warning" | "error";
  details: string;
}

const DEFAULT_STORE_ID = "01900000-0000-7000-8000-000000000001";
const SYNC_HISTORY_STORAGE_KEY = "openscale.syncHistory";
const MAX_SYNC_HISTORY = 50;

const INPUT_CLASS =
  "w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm text-foreground transition-colors placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20";

function triggerBlobDownload(data: BlobPart, filename: string, mimeType: string) {
  const blob = new Blob([data], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

function isScaleRegistered(candidate: DiscoveredScale, registered: Scale[]): boolean {
  return registered.some((scale) => {
    if (scale.ip_address === candidate.ip_address && scale.data_port === candidate.data_port) {
      return true;
    }
    if (
      candidate.serial_number &&
      scale.serial_number &&
      candidate.serial_number === scale.serial_number
    ) {
      return true;
    }
    if (candidate.mac_address && scale.mac_address && candidate.mac_address === scale.mac_address) {
      return true;
    }
    return false;
  });
}

function loadSyncHistory(): SyncHistoryEntry[] {
  try {
    const raw = window.localStorage.getItem(SYNC_HISTORY_STORAGE_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as SyncHistoryEntry[]) : [];
  } catch {
    return [];
  }
}

function saveSyncHistory(entries: SyncHistoryEntry[]) {
  try {
    window.localStorage.setItem(SYNC_HISTORY_STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // Ignore storage quota errors
  }
}

export function App() {
  const { t, i18n } = useTranslation();
  const transport = useOpenScaleTransport();
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const [activeTab, setActiveTab] = useState<Tab>("scales");
  const [darkMode, setDarkMode] = useState(() =>
    document.documentElement.classList.contains("dark"),
  );
  const [discovered, setDiscovered] = useState<DiscoveredScale[]>([]);
  const [syncHistory, setSyncHistory] = useState<SyncHistoryEntry[]>(() => loadSyncHistory());

  // Catalog state
  const [search, setSearch] = useState("");
  const [showForm, setShowForm] = useState(false);
  const [editingPlu, setEditingPlu] = useState<Plu | null>(null);
  const [isSavingPlu, setIsSavingPlu] = useState(false);
  const [showBulk, setShowBulk] = useState(false);
  const [bulkMode, setBulkMode] = useState<"percent" | "fixed_cents">("percent");
  const [bulkValue, setBulkValue] = useState("5");
  const [isApplyingBulk, setIsApplyingBulk] = useState(false);
  const [pendingDeletePlu, setPendingDeletePlu] = useState<Plu | null>(null);
  const [isDeletingPlu, setIsDeletingPlu] = useState(false);

  // Inline price editing state
  const [inlinePriceId, setInlinePriceId] = useState<string | null>(null);
  const [inlinePriceValue, setInlinePriceValue] = useState("");
  const [isSavingInlinePrice, setIsSavingInlinePrice] = useState(false);

  const scalesQuery = useQuery({
    queryKey: ["scales"],
    queryFn: () => transport.listScales(),
  });

  const plusQuery = useQuery({
    queryKey: ["plus"],
    queryFn: () => transport.listPlus(),
  });

  const scales = scalesQuery.data ?? [];
  const allPlus = plusQuery.data ?? [];

  const unregisteredDiscovered = useMemo(
    () => discovered.filter((item) => !isScaleRegistered(item, scales)),
    [discovered, scales],
  );

  const onlineScalesCount = useMemo(
    () => scales.filter((scale) => scale.status === "online").length,
    [scales],
  );

  const filteredPlus = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) {
      return allPlus;
    }
    return allPlus.filter(
      (p) =>
        p.description.toLowerCase().includes(q) ||
        String(p.key.plu).includes(q) ||
        (p.ean ?? "").toLowerCase().includes(q),
    );
  }, [allPlus, search]);

  const appendHistory = useCallback((entry: Omit<SyncHistoryEntry, "id" | "timestamp">) => {
    const fullEntry: SyncHistoryEntry = {
      ...entry,
      id: crypto.randomUUID(),
      timestamp: new Date().toISOString(),
    };
    setSyncHistory((prev) => {
      const next = [fullEntry, ...prev].slice(0, MAX_SYNC_HISTORY);
      saveSyncHistory(next);
      return next;
    });
  }, []);

  const clearHistory = () => {
    setSyncHistory([]);
    saveSyncHistory([]);
  };

  const getScaleLabel = (scaleId: string) => {
    const found = scales.find((s) => s.id === scaleId);
    return found ? `${found.name} (${found.ip_address})` : scaleId;
  };

  const discoverMutation = useMutation({
    mutationFn: () => transport.discoverScales(),
    onSuccess: (items) => {
      const newItems = items.filter((item) => !isScaleRegistered(item, scales));
      const existingCount = items.length - newItems.length;
      setDiscovered(newItems);

      let message: string;
      let variant: "success" | "info" = "info";
      if (items.length === 0) {
        message = t("notifications.lanNone");
      } else if (newItems.length === 0) {
        message = t("notifications.lanAllRegistered", { count: items.length });
      } else if (existingCount > 0) {
        variant = "success";
        message = t("notifications.lanFoundFiltered", {
          newCount: newItems.length,
          existingCount,
        });
      } else {
        variant = "success";
        message = t("notifications.lanFound", { count: newItems.length });
      }

      toast({ variant, message });
      appendHistory({
        operation: "discover",
        target: "LAN",
        status: "success",
        details: message,
      });
    },
    onError: (err) => {
      const message = t("notifications.error", { message: String(err) });
      toast({ variant: "error", message });
      appendHistory({
        operation: "discover",
        target: "LAN",
        status: "error",
        details: String(err),
      });
    },
  });

  const importFromScaleMutation = useMutation({
    mutationFn: (scaleId: string) => transport.importFromScale(scaleId),
    onSuccess: async (count, scaleId) => {
      await queryClient.invalidateQueries({ queryKey: ["plus"] });
      const message = t("notifications.importedFromScale", { count });
      toast({ variant: "success", message });
      appendHistory({
        operation: "import",
        target: getScaleLabel(scaleId),
        status: "success",
        details: message,
      });
    },
    onError: (err, scaleId) => {
      const message = t("notifications.error", { message: String(err) });
      toast({ variant: "error", message });
      appendHistory({
        operation: "import",
        target: getScaleLabel(scaleId),
        status: "error",
        details: String(err),
      });
    },
  });

  const syncScaleMutation = useMutation({
    mutationFn: ({ scaleId, mode }: { scaleId: string; mode: SyncMode }) =>
      transport.syncScale(scaleId, mode),
    onSuccess: (report, variables) => {
      const message = t("notifications.syncComplete", {
        mode: report.mode,
        sent: report.sent_plus,
        skipped: report.skipped_unchanged,
        failed: report.failed_plus,
      });
      const status = report.failed_plus > 0 ? "warning" : "success";
      toast({ variant: status, message });
      appendHistory({
        operation: variables.mode,
        target: getScaleLabel(variables.scaleId),
        status,
        details: message,
      });
    },
    onError: (err, variables) => {
      const message = t("notifications.error", { message: String(err) });
      toast({ variant: "error", message });
      appendHistory({
        operation: variables.mode,
        target: getScaleLabel(variables.scaleId),
        status: "error",
        details: String(err),
      });
    },
  });

  const toggleTheme = () => {
    setDarkMode((prev) => {
      const next = !prev;
      document.documentElement.classList.toggle("dark", next);
      window.localStorage.setItem("openscale.theme", next ? "dark" : "light");
      return next;
    });
  };

  const toggleLanguage = () => {
    const next = i18n.language === "en" ? "es" : "en";
    window.localStorage.setItem("openscale.lang", next);
    void i18n.changeLanguage(next);
  };

  const registerDiscovered = async (item: DiscoveredScale) => {
    try {
      const now = new Date().toISOString();
      const scale: Scale = {
        id: crypto.randomUUID(),
        group_id: null,
        name: item.model ?? item.ip_address,
        family: item.family,
        ip_address: item.ip_address,
        data_port: item.data_port,
        service_port: item.service_port,
        serial_number: item.serial_number ?? null,
        model: item.model ?? null,
        firmware_version: item.firmware_version ?? null,
        mac_address: item.mac_address ?? null,
        status: "online",
        last_seen_at: now,
        meta: {
          store_id: DEFAULT_STORE_ID,
          sync_version: 1,
          updated_at: now,
          deleted_at: null,
        },
      };
      await transport.upsertScale(scale);
      await queryClient.invalidateQueries({ queryKey: ["scales"] });
      toast({
        variant: "success",
        message: t("notifications.scaleSaved", { name: scale.name, ip: scale.ip_address }),
      });
    } catch (err) {
      toast({
        variant: "error",
        message: t("notifications.error", { message: String(err) }),
      });
    }
  };

  const openCreateForm = () => {
    setEditingPlu(null);
    setShowForm(true);
  };

  const openEditForm = (plu: Plu) => {
    setEditingPlu(plu);
    setShowForm(true);
  };

  const handleSavePlu = async (record: Plu) => {
    setIsSavingPlu(true);
    try {
      await transport.upsertPlu(record);
      await queryClient.invalidateQueries({ queryKey: ["plus"] });
      setShowForm(false);
      setEditingPlu(null);
      toast({
        variant: "success",
        message: t("notifications.pluSaved", { did: record.key.did, plu: record.key.plu }),
      });
    } catch (err) {
      toast({
        variant: "error",
        message: t("notifications.error", { message: String(err) }),
      });
    } finally {
      setIsSavingPlu(false);
    }
  };

  const startInlinePriceEdit = (plu: Plu) => {
    setInlinePriceId(plu.id);
    setInlinePriceValue((plu.price_1 / 100).toFixed(2));
  };

  const cancelInlinePriceEdit = () => {
    setInlinePriceId(null);
    setInlinePriceValue("");
  };

  const commitInlinePriceEdit = async (plu: Plu) => {
    const parsed = Number.parseFloat(inlinePriceValue);
    if (!Number.isFinite(parsed) || parsed < 0) {
      toast({ variant: "warning", message: t("notifications.invalidPrice") });
      return;
    }

    const nextCents = Math.round(parsed * 100);
    if (nextCents === plu.price_1) {
      cancelInlinePriceEdit();
      return;
    }

    setIsSavingInlinePrice(true);
    try {
      const updated: Plu = {
        ...plu,
        price_1: nextCents,
        meta: {
          ...plu.meta,
          sync_version: plu.meta.sync_version + 1,
          updated_at: new Date().toISOString(),
        },
      };
      await transport.upsertPlu(updated);
      await queryClient.invalidateQueries({ queryKey: ["plus"] });
      toast({
        variant: "success",
        message: t("notifications.pluPriceUpdated", {
          did: plu.key.did,
          plu: plu.key.plu,
          price: (nextCents / 100).toFixed(2),
        }),
      });
      cancelInlinePriceEdit();
    } catch (err) {
      toast({
        variant: "error",
        message: t("notifications.error", { message: String(err) }),
      });
    } finally {
      setIsSavingInlinePrice(false);
    }
  };

  const handleConfirmDeletePlu = async () => {
    if (!pendingDeletePlu) {
      return;
    }
    setIsDeletingPlu(true);
    try {
      await transport.deletePlu(pendingDeletePlu.key);
      await queryClient.invalidateQueries({ queryKey: ["plus"] });
      toast({
        variant: "success",
        message: t("notifications.pluDeleted", {
          did: pendingDeletePlu.key.did,
          plu: pendingDeletePlu.key.plu,
        }),
      });
      setPendingDeletePlu(null);
    } catch (err) {
      toast({
        variant: "error",
        message: t("notifications.error", { message: String(err) }),
      });
    } finally {
      setIsDeletingPlu(false);
    }
  };

  const handleBulkPriceApply = async (event?: FormEvent) => {
    event?.preventDefault();
    const parsedVal = Number.parseFloat(bulkValue);
    if (!Number.isFinite(parsedVal) || filteredPlus.length === 0) {
      return;
    }
    setIsApplyingBulk(true);
    try {
      const adjusted = applyBulkPriceAdjustment(filteredPlus, {
        mode: bulkMode,
        value: bulkMode === "percent" ? parsedVal : Math.round(parsedVal * 100),
      });
      for (const item of adjusted) {
        await transport.upsertPlu(item);
      }
      await queryClient.invalidateQueries({ queryKey: ["plus"] });
      setShowBulk(false);
      toast({
        variant: "success",
        message: t("notifications.bulkUpdated", { count: adjusted.length }),
      });
    } catch (err) {
      toast({
        variant: "error",
        message: t("notifications.error", { message: String(err) }),
      });
    } finally {
      setIsApplyingBulk(false);
    }
  };

  const handleImportFile = async (file: File) => {
    try {
      const isExcel = /\.xlsx?$/i.test(file.name);
      const parsed = isExcel
        ? parsePlusFromXlsxBuffer(await file.arrayBuffer(), DEFAULT_STORE_ID)
        : parsePlusFromCsv(await file.text(), DEFAULT_STORE_ID);

      for (const item of parsed) {
        await transport.upsertPlu(item);
      }
      await queryClient.invalidateQueries({ queryKey: ["plus"] });
      toast({
        variant: "success",
        message: t("notifications.importedFromFile", {
          count: parsed.length,
          filename: file.name,
        }),
      });
    } catch (err) {
      toast({
        variant: "error",
        message: t("notifications.error", { message: String(err) }),
      });
    }
  };

  const handleExportCsv = () => {
    const csv = serializePlusToCsv(filteredPlus);
    triggerBlobDownload(csv, "openscale-plus.csv", "text/csv;charset=utf-8");
    toast({
      variant: "info",
      message: t("notifications.exportedCsv", { count: filteredPlus.length }),
    });
  };

  const handleExportXlsx = () => {
    const bytes = exportPlusToXlsxBuffer(filteredPlus);
    triggerBlobDownload(
      bytes.buffer as ArrayBuffer,
      "openscale-plus.xlsx",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    );
    toast({
      variant: "info",
      message: t("notifications.exportedXlsx", { count: filteredPlus.length }),
    });
  };

  const discoveredColumns: ColumnDef<DiscoveredScale>[] = [
    {
      accessorKey: "family",
      header: t("scales.columns.protocol"),
      cell: ({ getValue }) => (
        <Badge variant="default">
          {getValue<string>() === "avery_berkel_xs" ? "XS / M-Series" : "XTi / XTRA"}
        </Badge>
      ),
    },
    {
      accessorKey: "ip_address",
      header: t("scales.columns.ip"),
      cell: ({ getValue }) => (
        <span className="font-mono text-xs tabular-nums">{getValue<string>()}</span>
      ),
    },
    {
      accessorKey: "data_port",
      header: t("scales.columns.port"),
      cell: ({ getValue }) => (
        <span className="font-mono text-xs tabular-nums">{getValue<number>()}</span>
      ),
    },
    {
      accessorKey: "model",
      header: t("scales.columns.model"),
      cell: ({ getValue }) => getValue<string>() ?? "—",
    },
    {
      accessorKey: "firmware_version",
      header: t("scales.columns.firmware"),
      cell: ({ getValue }) => (
        <span className="font-mono text-xs text-muted-foreground">{getValue<string>() ?? "—"}</span>
      ),
    },
    {
      id: "actions",
      header: "",
      enableSorting: false,
      cell: ({ row }) => (
        <div className="flex justify-end">
          <Button size="sm" variant="outline" onClick={() => void registerDiscovered(row.original)}>
            {t("common.save")}
          </Button>
        </div>
      ),
    },
  ];

  const scaleColumns: ColumnDef<Scale>[] = [
    {
      accessorKey: "name",
      header: t("scales.columns.name"),
      cell: ({ getValue }) => <span className="font-medium">{getValue<string>()}</span>,
    },
    {
      accessorKey: "ip_address",
      header: t("scales.columns.ip"),
      cell: ({ getValue }) => (
        <span className="font-mono text-xs tabular-nums">{getValue<string>()}</span>
      ),
    },
    {
      accessorKey: "model",
      header: t("scales.columns.model"),
      cell: ({ getValue }) => getValue<string>() ?? "—",
    },
    {
      accessorKey: "status",
      header: t("scales.columns.status"),
      cell: ({ getValue }) => {
        const status = getValue<Scale["status"]>();
        const variant = status === "online" ? "success" : status === "error" ? "danger" : "default";
        return <Badge variant={variant}>{t(`scales.status.${status}`)}</Badge>;
      },
    },
    {
      id: "syncActions",
      header: t("catalog.columns.actions"),
      enableSorting: false,
      cell: ({ row }) => {
        const scaleId = row.original.id;
        const isImportingThis =
          importFromScaleMutation.isPending && importFromScaleMutation.variables === scaleId;
        const isDeltaSyncingThis =
          syncScaleMutation.isPending &&
          syncScaleMutation.variables?.scaleId === scaleId &&
          syncScaleMutation.variables?.mode === "delta";
        const isFullSyncingThis =
          syncScaleMutation.isPending &&
          syncScaleMutation.variables?.scaleId === scaleId &&
          syncScaleMutation.variables?.mode === "full";

        return (
          <div className="flex flex-wrap gap-1.5">
            <Button
              size="sm"
              variant="outline"
              disabled={importFromScaleMutation.isPending}
              onClick={() => importFromScaleMutation.mutate(scaleId)}
            >
              <Download
                className={isImportingThis ? "h-3.5 w-3.5 animate-bounce" : "h-3.5 w-3.5"}
              />
              {isImportingThis ? t("scales.importing") : t("scales.importCatalog")}
            </Button>
            <Button
              size="sm"
              disabled={syncScaleMutation.isPending}
              onClick={() => syncScaleMutation.mutate({ scaleId, mode: "delta" })}
            >
              <RefreshCw
                className={isDeltaSyncingThis ? "h-3.5 w-3.5 animate-spin" : "h-3.5 w-3.5"}
              />
              {isDeltaSyncingThis ? t("scales.syncing") : t("scales.deltaSync")}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={syncScaleMutation.isPending}
              onClick={() => syncScaleMutation.mutate({ scaleId, mode: "full" })}
            >
              <RefreshCw
                className={isFullSyncingThis ? "h-3.5 w-3.5 animate-spin" : "h-3.5 w-3.5"}
              />
              {isFullSyncingThis ? t("scales.syncing") : t("scales.fullSync")}
            </Button>
          </div>
        );
      },
    },
  ];

  const pluColumns: ColumnDef<Plu>[] = [
    {
      id: "did",
      header: t("catalog.columns.did"),
      accessorFn: (row) => row.key.did,
      cell: ({ getValue }) => (
        <span className="font-mono text-xs text-muted-foreground tabular-nums">
          {getValue<number>()}
        </span>
      ),
    },
    {
      id: "plu",
      header: t("catalog.columns.plu"),
      accessorFn: (row) => row.key.plu,
      cell: ({ getValue }) => (
        <span className="font-mono font-medium tabular-nums">{getValue<number>()}</span>
      ),
    },
    {
      accessorKey: "description",
      header: t("catalog.columns.description"),
      cell: ({ getValue }) => <span className="font-medium">{getValue<string>()}</span>,
    },
    {
      id: "price1",
      header: t("catalog.columns.price1"),
      accessorFn: (row) => row.price_1,
      cell: ({ row }) => {
        const plu = row.original;
        const isEditing = inlinePriceId === plu.id;

        if (isEditing) {
          return (
            <div className="flex items-center gap-1">
              <div className="relative w-24">
                <span className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-xs text-muted-foreground">
                  $
                </span>
                <input
                  type="number"
                  step="0.01"
                  min={0}
                  value={inlinePriceValue}
                  disabled={isSavingInlinePrice}
                  onChange={(ev) => setInlinePriceValue(ev.target.value)}
                  onKeyDown={(ev) => {
                    if (ev.key === "Enter") {
                      ev.preventDefault();
                      void commitInlinePriceEdit(plu);
                    } else if (ev.key === "Escape") {
                      cancelInlinePriceEdit();
                    }
                  }}
                  className="w-full rounded border border-primary bg-background py-0.5 pr-1.5 pl-5 font-mono text-xs tabular-nums focus:outline-none focus:ring-2 focus:ring-primary/25"
                />
              </div>
              <Button
                size="icon"
                variant="ghost"
                className="h-6 w-6 text-emerald-600 dark:text-emerald-400"
                disabled={isSavingInlinePrice}
                onClick={() => void commitInlinePriceEdit(plu)}
                aria-label={t("common.save")}
              >
                <Check className="h-3.5 w-3.5" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                className="h-6 w-6 text-muted-foreground"
                disabled={isSavingInlinePrice}
                onClick={cancelInlinePriceEdit}
                aria-label={t("common.cancel")}
              >
                <X className="h-3.5 w-3.5" />
              </Button>
            </div>
          );
        }

        return (
          <button
            type="button"
            onClick={() => startInlinePriceEdit(plu)}
            title={t("catalog.clickToEditPrice")}
            className="group inline-flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 font-mono font-medium tabular-nums transition-colors hover:bg-muted"
          >
            <span>${(plu.price_1 / 100).toFixed(2)}</span>
            <Pencil className="h-3 w-3 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
          </button>
        );
      },
    },
    {
      accessorKey: "ean",
      header: t("catalog.columns.ean"),
      cell: ({ getValue }) => (
        <span className="font-mono text-xs text-muted-foreground tabular-nums">
          {getValue<string>() ?? "—"}
        </span>
      ),
    },
    {
      id: "actions",
      header: t("catalog.columns.actions"),
      enableSorting: false,
      cell: ({ row }) => (
        <div className="flex gap-1.5">
          <Button size="sm" variant="outline" onClick={() => openEditForm(row.original)}>
            {t("common.edit")}
          </Button>
          <Button
            size="icon"
            variant="ghost"
            onClick={() => setPendingDeletePlu(row.original)}
            aria-label={t("common.delete")}
            title={t("common.delete")}
          >
            <Trash2 className="h-3.5 w-3.5 text-destructive" />
          </Button>
        </div>
      ),
    },
  ];

  const syncHistoryColumns: ColumnDef<SyncHistoryEntry>[] = [
    {
      accessorKey: "timestamp",
      header: t("syncHistory.columns.time"),
      cell: ({ getValue }) => (
        <span className="font-mono text-xs text-muted-foreground tabular-nums">
          {new Date(getValue<string>()).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
            second: "2-digit",
          })}
        </span>
      ),
    },
    {
      accessorKey: "operation",
      header: t("syncHistory.columns.operation"),
      cell: ({ getValue }) => (
        <Badge variant="default">{t(`syncHistory.ops.${getValue<string>()}`)}</Badge>
      ),
    },
    {
      accessorKey: "target",
      header: t("syncHistory.columns.target"),
      cell: ({ getValue }) => <span className="font-medium">{getValue<string>()}</span>,
    },
    {
      accessorKey: "status",
      header: t("syncHistory.columns.status"),
      cell: ({ getValue }) => {
        const status = getValue<SyncHistoryEntry["status"]>();
        const variant =
          status === "success" ? "success" : status === "warning" ? "warning" : "danger";
        return <Badge variant={variant}>{t(`syncHistory.status.${status}`)}</Badge>;
      },
    },
    {
      accessorKey: "details",
      header: t("syncHistory.columns.details"),
      cell: ({ getValue }) => (
        <span className="text-xs text-muted-foreground">{getValue<string>()}</span>
      ),
    },
  ];

  return (
    <div className="flex min-h-screen bg-background text-foreground">
      <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-card p-4">
        <div className="mb-6 flex items-center gap-3 px-1">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-xs">
            <ScaleIcon className="h-5 w-5" />
          </div>
          <div className="min-w-0">
            <h1 className="text-base font-bold tracking-tight leading-tight">{t("app.name")}</h1>
            <p className="truncate text-[11px] text-muted-foreground" title={t("app.tagline")}>
              {t("app.tagline")}
            </p>
          </div>
        </div>

        <nav className="flex flex-col gap-1">
          <Button
            variant={activeTab === "scales" ? "default" : "ghost"}
            className="justify-between"
            onClick={() => setActiveTab("scales")}
          >
            <span className="flex items-center gap-2">
              <ScaleIcon className="h-4 w-4" />
              {t("nav.scales")}
            </span>
            <span
              className={
                activeTab === "scales"
                  ? "rounded-full bg-primary-foreground/20 px-2 py-0.5 text-[11px] font-semibold tabular-nums"
                  : "rounded-full bg-muted px-2 py-0.5 text-[11px] font-semibold text-muted-foreground tabular-nums"
              }
            >
              {scales.length}
            </span>
          </Button>
          <Button
            variant={activeTab === "catalog" ? "default" : "ghost"}
            className="justify-between"
            onClick={() => setActiveTab("catalog")}
          >
            <span className="flex items-center gap-2">
              <Database className="h-4 w-4" />
              {t("nav.catalog")}
            </span>
            <span
              className={
                activeTab === "catalog"
                  ? "rounded-full bg-primary-foreground/20 px-2 py-0.5 text-[11px] font-semibold tabular-nums"
                  : "rounded-full bg-muted px-2 py-0.5 text-[11px] font-semibold text-muted-foreground tabular-nums"
              }
            >
              {allPlus.length}
            </span>
          </Button>
          <Button
            variant={activeTab === "sync" ? "default" : "ghost"}
            className="justify-between"
            onClick={() => setActiveTab("sync")}
          >
            <span className="flex items-center gap-2">
              <History className="h-4 w-4" />
              {t("nav.sync")}
            </span>
            <span
              className={
                activeTab === "sync"
                  ? "rounded-full bg-primary-foreground/20 px-2 py-0.5 text-[11px] font-semibold tabular-nums"
                  : "rounded-full bg-muted px-2 py-0.5 text-[11px] font-semibold text-muted-foreground tabular-nums"
              }
            >
              {syncHistory.length}
            </span>
          </Button>
        </nav>

        <div className="mt-auto flex items-center gap-2 border-t border-border pt-4">
          <Button
            size="sm"
            variant="outline"
            className="flex-1"
            onClick={toggleLanguage}
            title={t("common.language")}
          >
            <Globe className="h-3.5 w-3.5" />
            {i18n.language.toUpperCase()}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={toggleTheme}
            aria-label={darkMode ? t("common.lightMode") : t("common.darkMode")}
            title={darkMode ? t("common.lightMode") : t("common.darkMode")}
          >
            {darkMode ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </Button>
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-6xl space-y-6">
          {activeTab === "scales" && (
            <>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <h2 className="text-xl font-semibold tracking-tight">{t("nav.scales")}</h2>
                <Button
                  onClick={() => discoverMutation.mutate()}
                  disabled={discoverMutation.isPending}
                >
                  <Radar
                    className={discoverMutation.isPending ? "h-4 w-4 animate-spin" : "h-4 w-4"}
                  />
                  {discoverMutation.isPending ? t("scales.discovering") : t("scales.discover")}
                </Button>
              </div>

              <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                <div className="rounded-lg border border-border bg-card px-4 py-3 shadow-xs">
                  <p className="text-xs font-medium text-muted-foreground">
                    {t("scales.stats.registered")}
                  </p>
                  <p className="mt-1 text-2xl font-bold tabular-nums">{scales.length}</p>
                </div>
                <div className="rounded-lg border border-border bg-card px-4 py-3 shadow-xs">
                  <p className="text-xs font-medium text-muted-foreground">
                    {t("scales.stats.online")}
                  </p>
                  <p className="mt-1 text-2xl font-bold text-emerald-600 tabular-nums dark:text-emerald-400">
                    {onlineScalesCount}
                  </p>
                </div>
                <div className="rounded-lg border border-border bg-card px-4 py-3 shadow-xs">
                  <p className="text-xs font-medium text-muted-foreground">
                    {t("scales.stats.discovered")}
                  </p>
                  <p className="mt-1 text-2xl font-bold text-primary tabular-nums">
                    {unregisteredDiscovered.length}
                  </p>
                </div>
              </div>

              {unregisteredDiscovered.length > 0 && (
                <section className="space-y-2.5">
                  <div className="flex items-center justify-between">
                    <h3 className="text-sm font-semibold text-muted-foreground">
                      {t("scales.lanDiscovery")}
                    </h3>
                    <Badge variant="default">{unregisteredDiscovered.length}</Badge>
                  </div>
                  <DataTable
                    columns={discoveredColumns}
                    data={unregisteredDiscovered}
                    emptyMessage={t("common.noData")}
                    sortable
                  />
                </section>
              )}

              <section className="space-y-2.5">
                <h3 className="text-sm font-semibold text-muted-foreground">
                  {t("scales.registered")}
                </h3>
                <DataTable
                  columns={scaleColumns}
                  data={scales}
                  loading={scalesQuery.isLoading}
                  emptyMessage={t("common.noData")}
                  emptyDescription={t("scales.emptyHint")}
                  emptyIcon={<ScaleIcon className="h-8 w-8" />}
                  sortable
                />
              </section>
            </>
          )}

          {activeTab === "catalog" && (
            <>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <h2 className="text-xl font-semibold tracking-tight">{t("catalog.title")}</h2>
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
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleExportCsv}
                    disabled={filteredPlus.length === 0}
                  >
                    <Download className="h-4 w-4" />
                    {t("catalog.exportCsv")}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleExportXlsx}
                    disabled={filteredPlus.length === 0}
                  >
                    <Download className="h-4 w-4" />
                    {t("catalog.exportXlsx")}
                  </Button>
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => setShowBulk(true)}
                    disabled={filteredPlus.length === 0}
                  >
                    <Percent className="h-4 w-4" />
                    {t("catalog.bulkPrice")}
                  </Button>
                  <Button size="sm" onClick={openCreateForm}>
                    <Plus className="h-4 w-4" />
                    {t("catalog.addPlu")}
                  </Button>
                </div>
              </div>

              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="relative w-full max-w-md">
                  <Search
                    className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground"
                    aria-hidden="true"
                  />
                  <input
                    type="search"
                    value={search}
                    onChange={(ev) => setSearch(ev.target.value)}
                    placeholder={t("catalog.searchPlaceholder")}
                    className="w-full rounded-md border border-border bg-card py-1.5 pr-8 pl-9 text-sm text-foreground transition-colors placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                  />
                  {search && (
                    <button
                      type="button"
                      onClick={() => setSearch("")}
                      aria-label={t("common.clearSearch")}
                      className="absolute top-1/2 right-2 inline-flex h-5 w-5 -translate-y-1/2 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  )}
                </div>
                <Badge variant="default" className="tabular-nums">
                  {search.trim()
                    ? t("catalog.filteredCount", {
                        shown: filteredPlus.length,
                        total: allPlus.length,
                      })
                    : t("catalog.pluCount", { count: allPlus.length })}
                </Badge>
              </div>

              <DataTable
                columns={pluColumns}
                data={filteredPlus}
                loading={plusQuery.isLoading}
                emptyMessage={t("common.noData")}
                emptyDescription={
                  search.trim() ? t("catalog.emptySearchHint") : t("catalog.emptyHint")
                }
                emptyIcon={<Database className="h-8 w-8" />}
                sortable
                initialSorting={[{ id: "plu", desc: false }]}
              />
            </>
          )}

          {activeTab === "sync" && (
            <>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <h2 className="text-xl font-semibold tracking-tight">{t("syncHistory.title")}</h2>
                  <p className="mt-0.5 text-xs text-muted-foreground">
                    {t("syncHistory.subtitle")}
                  </p>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={clearHistory}
                  disabled={syncHistory.length === 0}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  {t("syncHistory.clear")}
                </Button>
              </div>

              <DataTable
                columns={syncHistoryColumns}
                data={syncHistory}
                emptyMessage={t("common.noData")}
                emptyDescription={t("syncHistory.emptyHint")}
                emptyIcon={<History className="h-8 w-8" />}
                sortable
              />
            </>
          )}
        </div>
      </main>

      <PluFormDialog
        open={showForm}
        editingPlu={editingPlu}
        existingPlus={allPlus}
        storeId={DEFAULT_STORE_ID}
        isSaving={isSavingPlu}
        onClose={() => {
          setShowForm(false);
          setEditingPlu(null);
        }}
        onInvalidKey={() =>
          toast({
            variant: "warning",
            message: t("notifications.invalidDidPlu"),
          })
        }
        onSubmit={handleSavePlu}
      />

      <Dialog
        open={showBulk}
        onClose={() => setShowBulk(false)}
        title={t("catalog.bulkPrice")}
        description={t("catalog.bulkDescription", { count: filteredPlus.length })}
        size="sm"
        closeLabel={t("common.close")}
        footer={
          <>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setShowBulk(false)}
              disabled={isApplyingBulk}
            >
              {t("common.cancel")}
            </Button>
            <Button size="sm" type="submit" form="bulk-price-form" disabled={isApplyingBulk}>
              {t("common.apply")}
            </Button>
          </>
        }
      >
        <form
          id="bulk-price-form"
          onSubmit={(ev) => void handleBulkPriceApply(ev)}
          className="grid grid-cols-1 gap-3.5 text-sm sm:grid-cols-2"
        >
          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-muted-foreground">
              {t("catalog.columns.productType")}
            </span>
            <select
              value={bulkMode}
              onChange={(ev) => setBulkMode(ev.target.value as "percent" | "fixed_cents")}
              className={INPUT_CLASS}
            >
              <option value="percent">{t("catalog.bulkModePercent")}</option>
              <option value="fixed_cents">{t("catalog.bulkModeFixed")}</option>
            </select>
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-muted-foreground">
              {t("catalog.bulkValueLabel")}
            </span>
            <input
              type="number"
              step="0.5"
              required
              value={bulkValue}
              onChange={(ev) => setBulkValue(ev.target.value)}
              className={INPUT_CLASS}
            />
          </label>
        </form>
      </Dialog>

      <ConfirmDialog
        open={pendingDeletePlu !== null}
        onClose={() => setPendingDeletePlu(null)}
        onConfirm={() => void handleConfirmDeletePlu()}
        title={
          pendingDeletePlu
            ? t("catalog.deleteConfirmTitle", {
                did: pendingDeletePlu.key.did,
                plu: pendingDeletePlu.key.plu,
              })
            : ""
        }
        description={
          pendingDeletePlu
            ? t("catalog.deleteConfirmDescription", {
                description: pendingDeletePlu.description || `PLU ${pendingDeletePlu.key.plu}`,
              })
            : ""
        }
        confirmLabel={t("common.delete")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        isPending={isDeletingPlu}
      />
    </div>
  );
}
