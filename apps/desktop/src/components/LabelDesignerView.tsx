import {
  ABML_FONTS,
  LABEL_FIELD_TYPES,
  LABEL_MAX_SLOTS,
  type LabelFieldModel,
  type LabelFormat,
  type LabelFormatModel,
  type Plu,
  type Scale,
  createBlankLabelFormat,
  estimateLabelHeightMm,
  findNextFreePfid,
  findNextFreeSlot,
  getAbmlFontSpec,
  hundredthsToMm,
  labelFormatToModel,
  mmToHundredths,
  mmToTenths,
  modelToLabelFormat,
  resolveFieldPreviewText,
  rotationDegrees,
  tenthsToMm,
  validateLabelFormat,
} from "@openscale/client";
import { Badge, Button } from "@openscale/ui";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  CheckCircle2,
  Download,
  FilePlus2,
  Layers,
  Loader2,
  Printer,
  RefreshCw,
  RotateCw,
  Search,
  Send,
  Trash2,
  Upload,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

const PX_PER_MM = 5;
const DEFAULT_STORE_ID = "01900000-0000-7000-8000-000000000001";
const MANUAL_SCALE_ID = "__manual__";

interface LabelDesignerViewProps {
  scales: Scale[];
  plus: Plu[];
  onStatus: (msg: string) => void;
}

interface DragState {
  slot: number;
  mode: "move" | "resize";
  startClientX: number;
  startClientY: number;
  origX1: number;
  origY1: number;
  origX2: number;
  origY2: number;
}

function cloneFormatModel(fmt: LabelFormatModel): LabelFormatModel {
  return {
    ...fmt,
    fields: fmt.fields.map((f) => ({ ...f })),
    raw: [...fmt.raw],
  };
}

export function LabelDesignerView({ scales, plus, onStatus }: LabelDesignerViewProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const xsScales = useMemo(() => scales.filter((s) => s.family === "avery_berkel_xs"), [scales]);
  const defaultScaleId = useMemo(() => {
    const online = xsScales.find((s) => s.status === "online");
    return online?.id ?? xsScales[0]?.id ?? MANUAL_SCALE_ID;
  }, [xsScales]);

  const [selectedScaleId, setSelectedScaleId] = useState<string>(defaultScaleId);
  const [userChoseManual, setUserChoseManual] = useState(false);
  const [manualHost, setManualHost] = useState<string>(xsScales[0]?.ip_address ?? "192.168.9.113");

  const [scaleFormats, setScaleFormats] = useState<LabelFormatModel[] | null>(null);
  const [domainFormatsByPfid, setDomainFormatsByPfid] = useState<Record<number, LabelFormat>>({});
  const [isLoadingFormats, setIsLoadingFormats] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [formatSearch, setFormatSearch] = useState("");

  const [label, setLabel] = useState<LabelFormatModel>(() => createBlankLabelFormat(98, 59));
  const [heightMm, setHeightMm] = useState<number>(40);
  const [selectedSlot, setSelectedSlot] = useState<number | null>(null);
  const [isDirty, setIsDirty] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [previewPluId, setPreviewPluId] = useState<string>(plus[0]?.id ?? "");
  const [sampleWeightKg, setSampleWeightKg] = useState<number>(0.5);
  const dragRef = useRef<DragState | null>(null);
  const isDirtyRef = useRef(false);
  const activeRequestRef = useRef(0);

  isDirtyRef.current = isDirty;

  // Keep selectedScaleId synced if scales finish loading after mount
  useEffect(() => {
    const firstScale = xsScales[0];
    if (!firstScale) {
      return;
    }
    if (!userChoseManual && selectedScaleId === MANUAL_SCALE_ID) {
      const online = xsScales.find((s) => s.status === "online");
      setSelectedScaleId(online?.id ?? firstScale.id);
    } else if (
      selectedScaleId !== MANUAL_SCALE_ID &&
      !xsScales.some((s) => s.id === selectedScaleId)
    ) {
      setSelectedScaleId(firstScale.id);
    }
  }, [selectedScaleId, userChoseManual, xsScales]);

  const selectedScale = useMemo(
    () =>
      selectedScaleId === MANUAL_SCALE_ID
        ? null
        : (xsScales.find((s) => s.id === selectedScaleId) ?? null),
    [selectedScaleId, xsScales],
  );

  const activeHost = selectedScale?.ip_address ?? manualHost;
  const activeTargetLabel = selectedScale
    ? `${selectedScale.name} (${selectedScale.ip_address})`
    : manualHost;

  const localFormatsQuery = useQuery({
    queryKey: ["label_formats"],
    queryFn: () => invoke<LabelFormat[]>("list_label_formats"),
  });

  const localModels = useMemo(() => {
    const items = localFormatsQuery.data ?? [];
    return items.map(labelFormatToModel).sort((a, b) => a.pfid - b.pfid);
  }, [localFormatsQuery.data]);

  const displayedFormats = scaleFormats !== null ? scaleFormats : localModels;
  const formatSource: "scale" | "local" = scaleFormats !== null ? "scale" : "local";

  const filteredFormats = useMemo(() => {
    const q = formatSearch.trim().toLowerCase();
    if (!q) {
      return displayedFormats;
    }
    return displayedFormats.filter(
      (f) => String(f.pfid).includes(q) || f.name.toLowerCase().includes(q),
    );
  }, [displayedFormats, formatSearch]);

  const selectFormatIntoEditor = useCallback((fmt: LabelFormatModel) => {
    const cloned = cloneFormatModel(fmt);
    setLabel(cloned);
    setHeightMm(estimateLabelHeightMm(cloned.fields));
    setSelectedSlot(cloned.fields[0]?.slot ?? null);
    setIsDirty(false);
  }, []);

  const fetchScaleFormats = useCallback(
    async (notify = true) => {
      const requestId = ++activeRequestRef.current;
      setIsLoadingFormats(true);
      setLoadError(null);

      try {
        let models: LabelFormatModel[];
        const nextDomainMap: Record<number, LabelFormat> = {};

        if (selectedScale) {
          const domainFormats = await invoke<LabelFormat[]>("scale_list_label_formats", {
            scaleId: selectedScale.id,
          });
          for (const df of domainFormats) {
            nextDomainMap[df.pfid] = df;
          }
          models = domainFormats.map(labelFormatToModel).sort((a, b) => a.pfid - b.pfid);
        } else {
          const records = await invoke<LabelFormatModel[]>("xs_list_label_formats", {
            host: manualHost,
            port: null,
          });
          models = [...records].sort((a, b) => a.pfid - b.pfid);
        }

        if (requestId !== activeRequestRef.current) {
          return;
        }

        setDomainFormatsByPfid(nextDomainMap);
        setScaleFormats(models);

        const firstModel = models[0];
        if (firstModel && !isDirtyRef.current) {
          selectFormatIntoEditor(firstModel);
        }

        if (notify) {
          onStatus(
            models.length > 0
              ? t("labels.readSuccess", { count: models.length, target: activeTargetLabel })
              : t("labels.readEmpty", { target: activeTargetLabel }),
          );
        }
      } catch (err) {
        if (requestId !== activeRequestRef.current) {
          return;
        }
        const msg = String(err);
        setLoadError(msg);
        if (notify) {
          onStatus(t("labels.readError", { error: msg }));
        }
      } finally {
        if (requestId === activeRequestRef.current) {
          setIsLoadingFormats(false);
        }
      }
    },
    [activeTargetLabel, manualHost, onStatus, selectFormatIntoEditor, selectedScale, t],
  );

  // Automatically read existing formats from the selected registered scale on mount / scale switch
  useEffect(() => {
    if (!selectedScale) {
      return;
    }
    setScaleFormats(null);
    void fetchScaleFormats(false);
  }, [selectedScale, fetchScaleFormats]);

  const widthMm = Math.max(10, hundredthsToMm(label.width_hundredths_mm));
  const validation = useMemo(() => validateLabelFormat(label), [label]);
  const previewPlu = useMemo(
    () => plus.find((p) => p.id === previewPluId) ?? plus[0] ?? null,
    [plus, previewPluId],
  );
  const selectedField = useMemo(
    () => label.fields.find((f) => f.slot === selectedSlot) ?? null,
    [label.fields, selectedSlot],
  );

  const selectedFieldTypeLabel = useMemo(() => {
    if (!selectedField) {
      return "";
    }
    const meta = LABEL_FIELD_TYPES.find((item) => item.ft === selectedField.field_type);
    return meta
      ? `${meta.label} (FT ${selectedField.field_type})`
      : `FT ${selectedField.field_type}`;
  }, [selectedField]);

  const updateLabel = (updater: (prev: LabelFormatModel) => LabelFormatModel) => {
    setLabel(updater);
    setIsDirty(true);
  };

  const updateField = (slot: number, patch: Partial<LabelFieldModel>) => {
    updateLabel((prev) => ({
      ...prev,
      fields: prev.fields.map((f) => (f.slot === slot ? { ...f, ...patch } : f)),
    }));
  };

  const handleAddField = (ft: number) => {
    const nextSlot = findNextFreeSlot(label.fields);
    if (nextSlot === null) {
      onStatus(`Limit reached: maximum ${LABEL_MAX_SLOTS} field slots per XS label.`);
      return;
    }
    const meta = LABEL_FIELD_TYPES.find((item) => item.ft === ft);
    const wMm = Math.min(meta?.defaultWidthMm ?? 24, widthMm - 4);
    const hMm = Math.min(meta?.defaultHeightMm ?? 6, heightMm - 4);
    const offsetMm = (label.fields.length % 5) * 3 + 2;

    const newField: LabelFieldModel = {
      slot: nextSlot,
      field_type: ft,
      orientation: 0,
      style: 49,
      x1: mmToTenths(offsetMm),
      y1: mmToTenths(offsetMm),
      x2: mmToTenths(Math.min(widthMm, offsetMm + wMm)),
      y2: mmToTenths(Math.min(heightMm, offsetMm + hMm)),
      font: meta?.defaultFont ?? "2",
      dsr: ft === 10 || ft === 11 ? 1 : 0,
    };
    updateLabel((prev) => ({ ...prev, active: true, fields: [...prev.fields, newField] }));
    setSelectedSlot(nextSlot);
  };

  const handleDeleteField = (slot: number) => {
    updateLabel((prev) => ({
      ...prev,
      fields: prev.fields.filter((f) => f.slot !== slot),
    }));
    if (selectedSlot === slot) {
      setSelectedSlot(null);
    }
  };

  const handleNewBlankFormat = () => {
    const nextPfid = findNextFreePfid(
      displayedFormats.map((f) => f.pfid),
      98,
    );
    setLabel(createBlankLabelFormat(nextPfid, 59));
    setHeightMm(40);
    setSelectedSlot(null);
    setIsDirty(true);
  };

  const handleLoadSinglePfid = async () => {
    try {
      const rec = await invoke<LabelFormatModel>("xs_read_label_format", {
        host: activeHost,
        port: selectedScale?.data_port ?? null,
        pfid: label.pfid,
      });
      selectFormatIntoEditor(rec);
      onStatus(`Loaded PFID ${rec.pfid} from ${activeTargetLabel} (${rec.fields.length} fields).`);
    } catch (err) {
      onStatus(`Load PFID error: ${String(err)}`);
    }
  };

  const handleSendToScale = async () => {
    if (!validation.valid) {
      onStatus(`Cannot send label: ${validation.errors[0]}`);
      return;
    }
    setIsSaving(true);
    try {
      if (selectedScale) {
        const existingDomain =
          domainFormatsByPfid[label.pfid] ??
          localFormatsQuery.data?.find((f) => f.pfid === label.pfid) ??
          null;
        const domainPayload = modelToLabelFormat(label, DEFAULT_STORE_ID, existingDomain);
        const saved = await invoke<LabelFormat>("scale_write_label_format", {
          scaleId: selectedScale.id,
          fmt: domainPayload,
        });
        setDomainFormatsByPfid((prev) => ({ ...prev, [saved.pfid]: saved }));
        void queryClient.invalidateQueries({ queryKey: ["label_formats"] });
      } else {
        await invoke("xs_write_label_format", {
          host: manualHost,
          port: null,
          record: label,
        });
      }

      const savedSnapshot = cloneFormatModel(label);
      setScaleFormats((prev) => {
        if (prev === null) {
          return [savedSnapshot];
        }
        const exists = prev.some((f) => f.pfid === savedSnapshot.pfid);
        const next = exists
          ? prev.map((f) => (f.pfid === savedSnapshot.pfid ? savedSnapshot : f))
          : [...prev, savedSnapshot];
        return next.sort((a, b) => a.pfid - b.pfid);
      });
      setIsDirty(false);
      onStatus(
        t("labels.savedSuccess", {
          pfid: label.pfid,
          target: activeTargetLabel,
        }),
      );
    } catch (err) {
      onStatus(
        t("labels.savedError", {
          pfid: label.pfid,
          error: String(err),
        }),
      );
    } finally {
      setIsSaving(false);
    }
  };

  const handleTestPrint = async () => {
    try {
      const res = await invoke<string>("xs_service_action", {
        host: activeHost,
        port: selectedScale?.service_port ?? null,
        action: "print_test",
      });
      onStatus(
        res
          ? `Test print command sent to ${activeHost} (${res}).`
          : `Test print command sent to ${activeHost}.`,
      );
    } catch (err) {
      onStatus(`Test print error: ${String(err)}`);
    }
  };

  const handleExportJson = () => {
    const blob = new Blob([JSON.stringify(label, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `label-pfid-${label.pfid}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const onCanvasPointerMove = (ev: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag) {
      return;
    }
    const dxMm = Math.round(((ev.clientX - drag.startClientX) / PX_PER_MM) * 2) / 2;
    const dyMm = Math.round(((ev.clientY - drag.startClientY) / PX_PER_MM) * 2) / 2;
    const dxTenths = mmToTenths(Math.abs(dxMm)) * (dxMm < 0 ? -1 : 1);
    const dyTenths = mmToTenths(Math.abs(dyMm)) * (dyMm < 0 ? -1 : 1);
    const maxXTenths = mmToTenths(widthMm);
    const maxYTenths = mmToTenths(heightMm);

    if (drag.mode === "move") {
      const boxW = drag.origX2 - drag.origX1;
      const boxH = drag.origY2 - drag.origY1;
      const nextX1 = Math.max(0, Math.min(maxXTenths - boxW, drag.origX1 + dxTenths));
      const nextY1 = Math.max(0, Math.min(maxYTenths - boxH, drag.origY1 + dyTenths));
      updateField(drag.slot, {
        x1: nextX1,
        y1: nextY1,
        x2: nextX1 + boxW,
        y2: nextY1 + boxH,
      });
    } else {
      const nextX2 = Math.max(drag.origX1 + 20, Math.min(maxXTenths, drag.origX2 + dxTenths));
      const nextY2 = Math.max(drag.origY1 + 10, Math.min(maxYTenths, drag.origY2 + dyTenths));
      updateField(drag.slot, { x2: nextX2, y2: nextY2 });
    }
  };

  const stopDrag = () => {
    dragRef.current = null;
  };

  const hTicks = useMemo(() => {
    const ticks: number[] = [];
    for (let mm = 0; mm <= widthMm; mm += 5) {
      ticks.push(mm);
    }
    return ticks;
  }, [widthMm]);

  const vTicks = useMemo(() => {
    const ticks: number[] = [];
    for (let mm = 0; mm <= heightMm; mm += 5) {
      ticks.push(mm);
    }
    return ticks;
  }, [heightMm]);

  const hasCustomFont =
    selectedField !== null && !ABML_FONTS.some((f) => f.code === selectedField.font);

  return (
    <div className="space-y-4">
      {/* Top Toolbar */}
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-card p-3 text-sm">
        <div className="flex flex-wrap items-center gap-2">
          <select
            aria-label={t("labels.scaleSelector")}
            value={selectedScaleId}
            onChange={(ev) => {
              const val = ev.target.value;
              setSelectedScaleId(val);
              setUserChoseManual(val === MANUAL_SCALE_ID);
            }}
            className="rounded border border-border bg-background px-2 py-1 text-sm"
          >
            {xsScales.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name} ({s.ip_address})
              </option>
            ))}
            <option value={MANUAL_SCALE_ID}>{t("labels.manualIpOption")}</option>
          </select>

          {selectedScaleId === MANUAL_SCALE_ID && (
            <input
              type="text"
              value={manualHost}
              onChange={(ev) => setManualHost(ev.target.value)}
              placeholder="192.168.x.x"
              className="w-36 rounded border border-border bg-background px-2 py-1"
            />
          )}

          <Button
            size="sm"
            variant="secondary"
            disabled={isLoadingFormats}
            onClick={() => void fetchScaleFormats(true)}
          >
            {isLoadingFormats ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
            {isLoadingFormats ? t("labels.readingFromScale") : t("labels.readFromScale")}
          </Button>

          <div className="h-5 w-px bg-border" />

          <label className="flex items-center gap-1">
            <span className="text-xs text-muted-foreground">PFID</span>
            <input
              type="number"
              min={0}
              max={99}
              value={label.pfid}
              onChange={(ev) =>
                updateLabel((prev) => ({
                  ...prev,
                  pfid: Math.max(0, Math.min(99, Number.parseInt(ev.target.value, 10) || 0)),
                }))
              }
              className="w-16 rounded border border-border bg-background px-2 py-1"
            />
          </label>
          <Button size="sm" variant="outline" onClick={() => void handleLoadSinglePfid()}>
            <Upload className="h-3.5 w-3.5" /> {t("labels.loadPfid")}
          </Button>
          <Button size="sm" variant="outline" onClick={handleNewBlankFormat}>
            <FilePlus2 className="h-3.5 w-3.5" /> {t("labels.newFormat")}
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {isDirty && <Badge variant="warning">{t("labels.unsavedChanges")}</Badge>}
          <Badge variant={validation.valid ? "success" : "danger"}>
            <CheckCircle2 className="mr-1 h-3 w-3" />
            {validation.byteSize} B • {label.fields.length}/{LABEL_MAX_SLOTS}
          </Badge>
          <Button size="sm" variant="outline" onClick={handleExportJson}>
            <Download className="h-3.5 w-3.5" /> JSON
          </Button>
          <Button
            size="sm"
            disabled={!validation.valid || isSaving}
            onClick={() => void handleSendToScale()}
          >
            {isSaving ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Send className="h-3.5 w-3.5" />
            )}
            {isSaving ? t("labels.sendingToScale") : t("labels.sendToScale")}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            title="Print scale diagnostic test report (CMD 47)"
            onClick={() => void handleTestPrint()}
          >
            <Printer className="h-3.5 w-3.5" /> {t("labels.testPrint")}
          </Button>
        </div>
      </div>

      {/* Existing Formats on Scale */}
      <div className="rounded-md border border-border bg-card p-3">
        <div className="mb-2.5 flex flex-wrap items-center justify-between gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <Layers className="h-4 w-4 text-primary" />
            <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t("labels.scaleFormatsTitle")}
            </h3>
            <Badge variant={formatSource === "scale" ? "success" : "default"}>
              {formatSource === "scale" ? t("labels.sourceScale") : t("labels.sourceLocal")}
            </Badge>
            <span className="text-xs text-muted-foreground">
              {t("labels.formatsCount", { count: displayedFormats.length })}
            </span>
          </div>

          <div className="relative w-full max-w-xs">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <input
              type="search"
              value={formatSearch}
              onChange={(ev) => setFormatSearch(ev.target.value)}
              placeholder={t("labels.searchPlaceholder")}
              className="w-full rounded border border-border bg-background py-1 pr-2 pl-8 text-xs"
            />
          </div>
        </div>

        {loadError && (
          <div className="mb-2.5 flex items-center gap-2 rounded border border-destructive/40 bg-destructive/10 px-2.5 py-1.5 text-xs text-destructive">
            <AlertCircle className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate">{loadError}</span>
          </div>
        )}

        {isLoadingFormats && displayedFormats.length === 0 ? (
          <div className="flex items-center gap-2 py-4 text-xs text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            <span>{t("labels.readingFromScale")}</span>
          </div>
        ) : filteredFormats.length === 0 ? (
          <div className="rounded border border-dashed border-border px-3 py-4 text-center text-xs text-muted-foreground">
            <p className="font-medium">{t("labels.noFormatsFound")}</p>
            <p className="mt-0.5">{t("labels.noFormatsHint")}</p>
          </div>
        ) : (
          <div className="grid max-h-44 grid-cols-1 gap-2 overflow-y-auto pr-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {filteredFormats.map((fmt) => {
              const isCurrent = fmt.pfid === label.pfid;
              const fmtWidthMm = hundredthsToMm(fmt.width_hundredths_mm).toFixed(1);
              return (
                <button
                  key={fmt.pfid}
                  type="button"
                  onClick={() => selectFormatIntoEditor(fmt)}
                  className={`flex cursor-pointer flex-col items-start rounded-md border p-2 text-left transition-colors ${
                    isCurrent
                      ? "border-primary bg-primary/10 ring-1 ring-primary/40"
                      : "border-border bg-background hover:border-primary/50 hover:bg-muted/50"
                  }`}
                >
                  <div className="flex w-full items-center justify-between gap-1">
                    <span className="font-mono text-xs font-semibold text-primary">
                      PFID {fmt.pfid}
                    </span>
                    <div className="flex items-center gap-1">
                      {isCurrent && (
                        <Badge variant="default" className="px-1.5 py-0 text-[10px]">
                          {t("labels.editingBadge")}
                        </Badge>
                      )}
                      <Badge
                        variant={fmt.active ? "success" : "default"}
                        className="px-1.5 py-0 text-[10px]"
                      >
                        {fmt.active ? t("labels.active") : t("labels.inactive")}
                      </Badge>
                    </div>
                  </div>
                  <span className="mt-1 w-full truncate text-xs font-medium text-foreground">
                    {fmt.name.trim() || t("labels.unnamedFormat", { pfid: fmt.pfid })}
                  </span>
                  <span className="mt-0.5 text-[11px] text-muted-foreground">
                    {fmtWidthMm} mm • {t("labels.fieldsCount", { count: fmt.fields.length })}
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>

      {/* Live PLU Preview Selector */}
      <div className="flex flex-wrap items-center gap-3 rounded-md border border-border bg-card px-3 py-2 text-xs">
        <span className="font-medium text-muted-foreground">{t("labels.livePreview")}</span>
        <select
          value={previewPlu?.id ?? ""}
          onChange={(ev) => setPreviewPluId(ev.target.value)}
          className="max-w-xs rounded border border-border bg-background px-2 py-1 text-xs"
        >
          {plus.length === 0 && <option value="">Sample PLU ($70.00/kg)</option>}
          {plus.slice(0, 100).map((p) => (
            <option key={p.id} value={p.id}>
              {p.key.did}:{p.key.plu} — {p.description} (${(p.price_1 / 100).toFixed(2)})
            </option>
          ))}
        </select>
        <label className="flex items-center gap-1.5">
          <span className="text-muted-foreground">{t("labels.weightKg")}</span>
          <input
            type="number"
            step="0.05"
            min="0.05"
            value={sampleWeightKg}
            onChange={(ev) => setSampleWeightKg(Math.max(0.01, Number(ev.target.value) || 0.5))}
            className="w-20 rounded border border-border bg-background px-2 py-1"
          />
        </label>
        {!validation.valid && (
          <span className="text-destructive">{validation.errors.join(" • ")}</span>
        )}
      </div>

      {/* Main 3-Column WYSIWYG Workspace */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-12">
        {/* Left Palette */}
        <div className="space-y-1.5 rounded-md border border-border bg-card p-3 lg:col-span-3">
          <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {t("labels.addField")}
          </h3>
          <div className="grid grid-cols-2 gap-1.5 lg:grid-cols-1">
            {LABEL_FIELD_TYPES.map((item) => (
              <Button
                key={item.ft}
                size="sm"
                variant="outline"
                className="justify-start text-xs"
                onClick={() => handleAddField(item.ft)}
              >
                <span className="mr-1.5 font-mono text-[10px] text-muted-foreground">
                  {item.ft}
                </span>
                <span className="truncate">{item.label}</span>
              </Button>
            ))}
          </div>
        </div>

        {/* Center Canvas with mm Rulers */}
        <div className="flex flex-col items-center justify-start overflow-auto rounded-md border border-border bg-muted/40 p-6 lg:col-span-6">
          <div className="inline-block select-none">
            {/* Top Horizontal Ruler (mm) */}
            <div className="ml-7 flex items-end" style={{ width: widthMm * PX_PER_MM, height: 22 }}>
              <div className="relative h-full w-full border-b border-border text-[9px] text-muted-foreground">
                {hTicks.map((mm) => (
                  <div
                    key={mm}
                    className="absolute bottom-0 border-l border-border"
                    style={{
                      left: mm * PX_PER_MM,
                      height: mm % 10 === 0 ? 12 : 6,
                    }}
                  >
                    {mm % 10 === 0 && <span className="ml-0.5">{mm}</span>}
                  </div>
                ))}
              </div>
            </div>

            <div className="flex">
              {/* Left Vertical Ruler (mm) */}
              <div
                className="relative w-7 border-r border-border text-[9px] text-muted-foreground"
                style={{ height: heightMm * PX_PER_MM }}
              >
                {vTicks.map((mm) => (
                  <div
                    key={mm}
                    className="absolute right-0 border-t border-border"
                    style={{
                      top: mm * PX_PER_MM,
                      width: mm % 10 === 0 ? 12 : 6,
                    }}
                  >
                    {mm % 10 === 0 && (
                      <span className="absolute -left-5 -top-1.5 text-[9px]">{mm}</span>
                    )}
                  </div>
                ))}
              </div>

              {/* Label Paper Surface */}
              <div
                className="relative overflow-hidden rounded-xs border border-slate-400 bg-white text-slate-900 shadow-md"
                style={{
                  width: widthMm * PX_PER_MM,
                  height: heightMm * PX_PER_MM,
                }}
                onPointerMove={onCanvasPointerMove}
                onPointerUp={stopDrag}
                onPointerLeave={stopDrag}
                onPointerDown={() => setSelectedSlot(null)}
              >
                {label.fields.map((field) => {
                  const left = tenthsToMm(field.x1) * PX_PER_MM;
                  const top = tenthsToMm(field.y1) * PX_PER_MM;
                  const w = Math.max(12, tenthsToMm(field.x2 - field.x1) * PX_PER_MM);
                  const h = Math.max(8, tenthsToMm(field.y2 - field.y1) * PX_PER_MM);
                  const fontSpec = getAbmlFontSpec(field.font);
                  const isSelected = field.slot === selectedSlot;
                  const text = resolveFieldPreviewText(field, previewPlu, sampleWeightKg);

                  return (
                    <div
                      key={field.slot}
                      onPointerDown={(ev) => {
                        ev.stopPropagation();
                        setSelectedSlot(field.slot);
                        dragRef.current = {
                          slot: field.slot,
                          mode: "move",
                          startClientX: ev.clientX,
                          startClientY: ev.clientY,
                          origX1: field.x1,
                          origY1: field.y1,
                          origX2: field.x2,
                          origY2: field.y2,
                        };
                      }}
                      className={`absolute cursor-move overflow-hidden ${
                        isSelected
                          ? "ring-2 ring-primary ring-offset-1"
                          : "hover:ring-1 hover:ring-slate-400"
                      } ${field.field_type === 58 ? "border-2 border-slate-900" : ""} ${
                        field.field_type === 57 ? "bg-slate-900" : ""
                      }`}
                      style={{
                        left,
                        top,
                        width: w,
                        height: h,
                        transform: `rotate(${rotationDegrees(field.orientation)}deg)`,
                        transformOrigin: "center center",
                      }}
                    >
                      {field.field_type === 10 ? (
                        <div className="flex h-full w-full flex-col items-center justify-between px-1 py-0.5">
                          <div
                            className="h-3/4 w-full"
                            style={{
                              backgroundImage:
                                "repeating-linear-gradient(90deg, #0f172a 0px, #0f172a 2px, transparent 2px, transparent 4px, #0f172a 4px, #0f172a 5px, transparent 5px, transparent 8px)",
                            }}
                          />
                          <span className="font-mono text-[9px] leading-none">{text}</span>
                        </div>
                      ) : (
                        <div
                          className="flex h-full w-full items-center whitespace-nowrap px-0.5 leading-none"
                          style={{
                            fontSize: fontSpec.pxSize,
                            fontWeight: fontSpec.bold ? 700 : 400,
                          }}
                        >
                          {text}
                        </div>
                      )}

                      {isSelected && (
                        <div
                          onPointerDown={(ev) => {
                            ev.stopPropagation();
                            dragRef.current = {
                              slot: field.slot,
                              mode: "resize",
                              startClientX: ev.clientX,
                              startClientY: ev.clientY,
                              origX1: field.x1,
                              origY1: field.y1,
                              origX2: field.x2,
                              origY2: field.y2,
                            };
                          }}
                          className="absolute bottom-0 right-0 h-2.5 w-2.5 cursor-nwse-resize bg-primary"
                        />
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>

        {/* Right Property Inspector */}
        <div className="space-y-4 rounded-md border border-border bg-card p-3 text-xs lg:col-span-3">
          <div className="space-y-2 border-b border-border pb-3">
            <h3 className="font-semibold uppercase tracking-wide text-muted-foreground">
              {t("labels.labelFormatSection")}
            </h3>
            <label className="flex flex-col gap-1">
              <span>{t("labels.formatName")}</span>
              <input
                type="text"
                maxLength={27}
                value={label.name}
                onChange={(ev) => updateLabel((prev) => ({ ...prev, name: ev.target.value }))}
                className="rounded border border-border bg-background px-2 py-1"
              />
            </label>
            <div className="grid grid-cols-2 gap-2">
              <label className="flex flex-col gap-1">
                <span>{t("labels.widthMm")}</span>
                <input
                  type="number"
                  min={10}
                  max={100}
                  value={widthMm}
                  onChange={(ev) =>
                    updateLabel((prev) => ({
                      ...prev,
                      width_hundredths_mm: mmToHundredths(Number(ev.target.value) || 59),
                    }))
                  }
                  className="rounded border border-border bg-background px-2 py-1"
                />
              </label>
              <label className="flex flex-col gap-1">
                <span>{t("labels.heightMm")}</span>
                <input
                  type="number"
                  min={20}
                  max={120}
                  value={heightMm}
                  onChange={(ev) => setHeightMm(Math.max(20, Number(ev.target.value) || 40))}
                  className="rounded border border-border bg-background px-2 py-1"
                />
              </label>
            </div>
            <div className="flex items-center justify-between pt-1">
              <span>{t("labels.orientation")}</span>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  updateLabel((prev) => ({
                    ...prev,
                    orientation: (prev.orientation + 1) % 4,
                  }))
                }
              >
                <RotateCw className="h-3 w-3" /> {rotationDegrees(label.orientation)}°
              </Button>
            </div>
          </div>

          {selectedField ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-1">
                <div className="min-w-0">
                  <h3 className="truncate font-semibold uppercase tracking-wide text-muted-foreground">
                    Slot #{selectedField.slot}
                  </h3>
                  <p className="truncate text-[11px] text-foreground">{selectedFieldTypeLabel}</p>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => handleDeleteField(selectedField.slot)}
                >
                  <Trash2 className="h-3.5 w-3.5 text-destructive" />
                </Button>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <label className="flex flex-col gap-1">
                  <span>X1 (mm)</span>
                  <input
                    type="number"
                    step="0.5"
                    value={tenthsToMm(selectedField.x1)}
                    onChange={(ev) =>
                      updateField(selectedField.slot, { x1: mmToTenths(Number(ev.target.value)) })
                    }
                    className="rounded border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>Y1 (mm)</span>
                  <input
                    type="number"
                    step="0.5"
                    value={tenthsToMm(selectedField.y1)}
                    onChange={(ev) =>
                      updateField(selectedField.slot, { y1: mmToTenths(Number(ev.target.value)) })
                    }
                    className="rounded border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>X2 (mm)</span>
                  <input
                    type="number"
                    step="0.5"
                    value={tenthsToMm(selectedField.x2)}
                    onChange={(ev) =>
                      updateField(selectedField.slot, { x2: mmToTenths(Number(ev.target.value)) })
                    }
                    className="rounded border border-border bg-background px-2 py-1"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span>Y2 (mm)</span>
                  <input
                    type="number"
                    step="0.5"
                    value={tenthsToMm(selectedField.y2)}
                    onChange={(ev) =>
                      updateField(selectedField.slot, { y2: mmToTenths(Number(ev.target.value)) })
                    }
                    className="rounded border border-border bg-background px-2 py-1"
                  />
                </label>
              </div>

              <label className="flex flex-col gap-1">
                <span>ABML Font (FF)</span>
                <select
                  value={selectedField.font}
                  onChange={(ev) => updateField(selectedField.slot, { font: ev.target.value })}
                  className="rounded border border-border bg-background px-2 py-1"
                >
                  {hasCustomFont && (
                    <option value={selectedField.font}>
                      {selectedField.font} — Custom (Scale)
                    </option>
                  )}
                  {ABML_FONTS.map((f) => (
                    <option key={f.code} value={f.code}>
                      {f.label}
                    </option>
                  ))}
                </select>
              </label>

              <div className="grid grid-cols-2 gap-2">
                <div className="flex flex-col gap-1">
                  <span>Rotation</span>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() =>
                      updateField(selectedField.slot, {
                        orientation: (selectedField.orientation + 1) % 4,
                      })
                    }
                  >
                    <RotateCw className="h-3 w-3" /> {rotationDegrees(selectedField.orientation)}°
                  </Button>
                </div>
                <label className="flex flex-col gap-1">
                  <span>DSR (BID/Msg/Logo)</span>
                  <input
                    type="number"
                    min={0}
                    max={99}
                    value={selectedField.dsr}
                    onChange={(ev) =>
                      updateField(selectedField.slot, {
                        dsr: Math.max(0, Number.parseInt(ev.target.value, 10) || 0),
                      })
                    }
                    className="rounded border border-border bg-background px-2 py-1"
                  />
                </label>
              </div>
            </div>
          ) : (
            <p className="text-muted-foreground">{t("labels.clickFieldHint")}</p>
          )}
        </div>
      </div>
    </div>
  );
}
