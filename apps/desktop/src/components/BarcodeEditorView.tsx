import {
  BARCODE_CATEGORIES,
  BARCODE_SYMBOLOGIES,
  BARCODE_TEMPLATE_TOKENS,
  type BarcodeFormat,
  type Plu,
  type Scale,
  type XsBarcodeFormatDef,
  encodeBarcodeToBars,
  parseBarcodeDefinition,
  resolveBarcodeTemplate,
} from "@openscale/client";
import { Badge, Button } from "@openscale/ui";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  Barcode,
  CheckCircle2,
  Loader2,
  Plus,
  RefreshCw,
  Save,
  Send,
} from "lucide-react";
import { useMemo, useState } from "react";

const DEFAULT_STORE_ID = "01900000-0000-7000-8000-000000000001";

interface BarcodeEditorViewProps {
  scales: Scale[];
  plus: Plu[];
  onStatus: (msg: string) => void;
}

export function BarcodeEditorView({ scales, plus, onStatus }: BarcodeEditorViewProps) {
  const queryClient = useQueryClient();
  const xsScales = useMemo(() => scales.filter((s) => s.family === "avery_berkel_xs"), [scales]);
  const [selectedScaleId, setSelectedScaleId] = useState<string>(xsScales[0]?.id ?? "");
  const [scaleFormats, setScaleFormats] = useState<BarcodeFormat[] | null>(null);
  const [isLoadingScale, setIsLoadingScale] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  // Editor state
  const [bid, setBid] = useState<number>(1);
  const [name, setName] = useState<string>("EAN-13 Peso");
  const [def, setDef] = useState<XsBarcodeFormatDef>({
    bmc: 0,
    bt: 3,
    bpd: 1,
    subtype: 3,
    template: "2NNAAAAWWWWWC",
  });
  const [editingId, setEditingId] = useState<string | null>(null);

  // Live preview controls
  const [previewPluId, setPreviewPluId] = useState<string>(plus[0]?.id ?? "");
  const [sampleWeightKg, setSampleWeightKg] = useState<number>(0.5);

  const localQuery = useQuery({
    queryKey: ["barcode_formats"],
    queryFn: () => invoke<BarcodeFormat[]>("list_barcode_formats"),
  });

  const displayedFormats = useMemo(() => {
    const list = scaleFormats !== null ? scaleFormats : (localQuery.data ?? []);
    return [...list].sort((a, b) => a.bid - b.bid);
  }, [scaleFormats, localQuery.data]);

  const previewPlu = useMemo(
    () => plus.find((p) => p.id === previewPluId) ?? plus[0] ?? null,
    [plus, previewPluId],
  );

  const resolved = useMemo(
    () => resolveBarcodeTemplate(def, previewPlu, sampleWeightKg),
    [def, previewPlu, sampleWeightKg],
  );

  const svgLayout = useMemo(
    () => encodeBarcodeToBars(resolved.code, def.bt),
    [resolved.code, def.bt],
  );

  const selectFormat = (fmt: BarcodeFormat) => {
    setEditingId(fmt.id);
    setBid(fmt.bid);
    setName(fmt.name);
    setDef(parseBarcodeDefinition(fmt));
  };

  const handleNew = () => {
    const used = new Set(displayedFormats.map((f) => f.bid));
    let nextBid = 1;
    for (let i = 1; i <= 32; i++) {
      if (!used.has(i)) {
        nextBid = i;
        break;
      }
    }
    setEditingId(null);
    setBid(nextBid);
    setName(`Código #${nextBid}`);
    setDef({ bmc: 0, bt: 3, bpd: 1, subtype: 3, template: "2NNAAAAWWWWWC" });
  };

  const handleReadFromScale = async () => {
    if (!selectedScaleId) {
      return;
    }
    setIsLoadingScale(true);
    try {
      const items = await invoke<BarcodeFormat[]>("scale_list_barcode_formats", {
        scaleId: selectedScaleId,
      });
      setScaleFormats(items);
      if (items[0]) {
        selectFormat(items[0]);
      }
      onStatus(`Se leyeron ${items.length} formato(s) de código de barras de la báscula.`);
    } catch (err) {
      onStatus(`Error al leer códigos de barras: ${String(err)}`);
    } finally {
      setIsLoadingScale(false);
    }
  };

  const buildPayload = (): BarcodeFormat => ({
    id: editingId ?? crypto.randomUUID(),
    bid: Math.max(1, Math.min(32, bid)),
    name: name.trim() || `Barcode ${bid}`,
    definition_json: JSON.stringify(def),
    meta: {
      store_id: DEFAULT_STORE_ID,
      sync_version: 1,
      updated_at: new Date().toISOString(),
      deleted_at: null,
    },
  });

  const handleSaveLocal = async () => {
    setIsSaving(true);
    try {
      const payload = buildPayload();
      await invoke("upsert_barcode_format", { fmt: payload });
      await queryClient.invalidateQueries({ queryKey: ["barcode_formats"] });
      setEditingId(payload.id);
      onStatus(`Formato BID ${payload.bid} guardado en catálogo local.`);
    } catch (err) {
      onStatus(`Error al guardar BID: ${String(err)}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleWriteToScale = async () => {
    if (!selectedScaleId) {
      onStatus("Selecciona una báscula XS primero.");
      return;
    }
    setIsSaving(true);
    try {
      const payload = buildPayload();
      const saved = await invoke<BarcodeFormat>("scale_write_barcode_format", {
        scaleId: selectedScaleId,
        fmt: payload,
      });
      await queryClient.invalidateQueries({ queryKey: ["barcode_formats"] });
      setScaleFormats((prev) => {
        if (!prev) {
          return null;
        }
        const exists = prev.some((f) => f.bid === saved.bid);
        return exists
          ? prev.map((f) => (f.bid === saved.bid ? saved : f))
          : [...prev, saved].sort((a, b) => a.bid - b.bid);
      });
      onStatus(`Formato BID ${saved.bid} enviado y guardado en la báscula.`);
    } catch (err) {
      onStatus(`Error al enviar BID a la báscula: ${String(err)}`);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Top Toolbar */}
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-card p-3 text-sm">
        <div className="flex flex-wrap items-center gap-2">
          <Barcode className="h-4 w-4 text-primary" />
          <select
            value={selectedScaleId}
            onChange={(ev) => setSelectedScaleId(ev.target.value)}
            className="rounded border border-border bg-background px-2 py-1 text-sm"
          >
            <option value="">Solo catálogo local</option>
            {xsScales.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name} ({s.ip_address})
              </option>
            ))}
          </select>
          <Button
            size="sm"
            variant="secondary"
            disabled={!selectedScaleId || isLoadingScale}
            onClick={() => void handleReadFromScale()}
          >
            {isLoadingScale ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
            Leer de báscula
          </Button>
          <Button size="sm" variant="outline" onClick={handleNew}>
            <Plus className="h-3.5 w-3.5" /> Nuevo BID
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            disabled={isSaving}
            onClick={() => void handleSaveLocal()}
          >
            <Save className="h-3.5 w-3.5" /> Guardar local
          </Button>
          <Button
            size="sm"
            disabled={isSaving || !selectedScaleId}
            onClick={() => void handleWriteToScale()}
          >
            {isSaving ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Send className="h-3.5 w-3.5" />
            )}
            Enviar a báscula
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-12">
        {/* Left: List of BIDs */}
        <div className="space-y-2 rounded-md border border-border bg-card p-3 lg:col-span-3">
          <div className="flex items-center justify-between">
            <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Formatos (BID 1..32)
            </h3>
            <Badge variant={scaleFormats !== null ? "success" : "default"}>
              {scaleFormats !== null ? "Báscula" : "Local"}
            </Badge>
          </div>
          <div className="max-h-[460px] space-y-1.5 overflow-y-auto pr-1">
            {displayedFormats.length === 0 ? (
              <p className="py-4 text-center text-xs text-muted-foreground">
                Sin formatos cargados. Lee de una báscula o crea uno nuevo.
              </p>
            ) : (
              displayedFormats.map((fmt) => {
                const parsed = parseBarcodeDefinition(fmt);
                const isCurrent = fmt.bid === bid;
                return (
                  <button
                    key={`${fmt.id}-${fmt.bid}`}
                    type="button"
                    onClick={() => selectFormat(fmt)}
                    className={`flex w-full cursor-pointer flex-col items-start rounded-md border p-2 text-left transition-colors ${
                      isCurrent
                        ? "border-primary bg-primary/10 ring-1 ring-primary/40"
                        : "border-border bg-background hover:bg-muted/40"
                    }`}
                  >
                    <div className="flex w-full items-center justify-between">
                      <span className="font-mono text-xs font-semibold text-primary">
                        BID {fmt.bid}
                      </span>
                      <span className="font-mono text-[10px] text-muted-foreground">
                        {BARCODE_SYMBOLOGIES.find((s) => s.value === parsed.bt)?.label ??
                          `BT ${parsed.bt}`}
                      </span>
                    </div>
                    <span className="mt-0.5 truncate text-xs font-medium">{fmt.name}</span>
                    <span className="font-mono text-[11px] text-muted-foreground">
                      {parsed.template}
                    </span>
                  </button>
                );
              })
            )}
          </div>
        </div>

        {/* Center/Right: Editor + Live SVG Viewer */}
        <div className="space-y-4 lg:col-span-9">
          {/* Structured Parameters */}
          <div className="grid grid-cols-1 gap-3 rounded-md border border-border bg-card p-4 text-sm sm:grid-cols-4">
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">BID (1..32)</span>
              <input
                type="number"
                min={1}
                max={32}
                value={bid}
                onChange={(ev) =>
                  setBid(Math.max(1, Math.min(32, Number.parseInt(ev.target.value, 10) || 1)))
                }
                className="rounded border border-border bg-background px-2.5 py-1.5 font-mono"
              />
            </label>
            <label className="flex flex-col gap-1 sm:col-span-3">
              <span className="text-xs font-medium text-muted-foreground">Nombre</span>
              <input
                type="text"
                value={name}
                onChange={(ev) => setName(ev.target.value)}
                className="rounded border border-border bg-background px-2.5 py-1.5"
              />
            </label>

            <label className="flex flex-col gap-1 sm:col-span-2">
              <span className="text-xs font-medium text-muted-foreground">Simbología (BT)</span>
              <select
                value={def.bt}
                onChange={(ev) => setDef({ ...def, bt: Number(ev.target.value) })}
                className="rounded border border-border bg-background px-2.5 py-1.5"
              >
                {BARCODE_SYMBOLOGIES.map((s) => (
                  <option key={s.value} value={s.value}>
                    {s.value} — {s.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">Categoría (BMC)</span>
              <select
                value={def.bmc}
                onChange={(ev) => setDef({ ...def, bmc: Number(ev.target.value) })}
                className="rounded border border-border bg-background px-2.5 py-1.5"
              >
                {BARCODE_CATEGORIES.map((c) => (
                  <option key={c.value} value={c.value}>
                    {c.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                Divisor Precio (BPD)
              </span>
              <select
                value={def.bpd}
                onChange={(ev) => setDef({ ...def, bpd: Number(ev.target.value) })}
                className="rounded border border-border bg-background px-2.5 py-1.5 font-mono"
              >
                <option value={1}>1 (centavos)</option>
                <option value={10}>10 (décimas)</option>
                <option value={100}>100 (enteros)</option>
              </select>
            </label>

            {/* Template Builder */}
            <div className="space-y-2 sm:col-span-4">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-muted-foreground">
                  Plantilla BFD ({def.template.length} caracteres)
                </span>
                <div className="flex flex-wrap gap-1">
                  {BARCODE_TEMPLATE_TOKENS.map((t) => (
                    <button
                      key={t.token}
                      type="button"
                      onClick={() => setDef({ ...def, template: `${def.template}${t.token}` })}
                      className="cursor-pointer rounded border border-border bg-background px-1.5 py-0.5 text-[11px] font-medium hover:bg-muted"
                      title={`Insertar ${t.label}`}
                    >
                      <span className="font-mono font-bold" style={{ color: t.color }}>
                        {t.token}
                      </span>{" "}
                      {t.label}
                    </button>
                  ))}
                </div>
              </div>
              <input
                type="text"
                value={def.template}
                onChange={(ev) =>
                  setDef({ ...def, template: ev.target.value.toUpperCase().replace(/\s+/g, "") })
                }
                placeholder="2NNAAAAWWWWWC"
                className="w-full rounded border border-border bg-background px-3 py-2 font-mono text-base tracking-widest"
              />
              {resolved.warnings.length > 0 ? (
                <div className="flex items-center gap-1.5 text-xs text-amber-600 dark:text-amber-400">
                  <AlertCircle className="h-3.5 w-3.5 shrink-0" />
                  <span>{resolved.warnings.join(" · ")}</span>
                </div>
              ) : (
                <div className="flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-400">
                  <CheckCircle2 className="h-3.5 w-3.5 shrink-0" />
                  <span>Plantilla válida para {resolved.symbology}.</span>
                </div>
              )}
            </div>
          </div>

          {/* Live SVG Barcode Viewer */}
          <div className="space-y-3 rounded-md border border-border bg-card p-4">
            <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
              <span className="font-semibold uppercase tracking-wide text-muted-foreground">
                Visor en Vivo ({resolved.symbology})
              </span>
              <div className="flex flex-wrap items-center gap-2">
                <select
                  value={previewPlu?.id ?? ""}
                  onChange={(ev) => setPreviewPluId(ev.target.value)}
                  className="max-w-xs rounded border border-border bg-background px-2 py-1 text-xs"
                >
                  {plus.length === 0 && <option value="">PLU ejemplo ($70.00/kg)</option>}
                  {plus.slice(0, 100).map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.key.did}:{p.key.plu} — {p.description} (${(p.price_1 / 100).toFixed(2)})
                    </option>
                  ))}
                </select>
                <label className="flex items-center gap-1">
                  <span className="text-muted-foreground">Peso (kg):</span>
                  <input
                    type="number"
                    step="0.05"
                    min="0.05"
                    value={sampleWeightKg}
                    onChange={(ev) =>
                      setSampleWeightKg(Math.max(0.01, Number(ev.target.value) || 0.5))
                    }
                    className="w-20 rounded border border-border bg-background px-2 py-1 font-mono"
                  />
                </label>
              </div>
            </div>

            <div className="flex flex-col items-center justify-center rounded-md border border-slate-300 bg-white p-6 text-slate-900">
              <svg
                viewBox={`-6 0 ${svgLayout.totalModules + 12} 44`}
                className="h-28 w-full max-w-md"
                preserveAspectRatio="none"
                role="img"
                aria-label={`Código de barras ${resolved.code}`}
              >
                {svgLayout.bars.map((bar) => (
                  <rect key={bar.x} x={bar.x} y={0} width={bar.width} height={44} fill="#0f172a" />
                ))}
              </svg>
              <div className="mt-2 flex flex-wrap justify-center gap-0.5 font-mono text-lg font-bold tracking-widest">
                {resolved.segments.map((seg, i) => (
                  <span
                    key={`${seg.token}-${i}`}
                    style={{ color: seg.color }}
                    title={`Token ${seg.token}`}
                    className="rounded px-0.5"
                  >
                    {seg.value}
                  </span>
                ))}
              </div>
              <div className="mt-2 flex flex-wrap justify-center gap-2 text-[11px] text-slate-600">
                {resolved.segments.map((seg, i) => (
                  <span key={`legend-${seg.token}-${i}`} className="flex items-center gap-1">
                    <span
                      className="inline-block h-2 w-2 rounded-full"
                      style={{ backgroundColor: seg.color }}
                    />
                    <span className="font-mono font-semibold">{seg.token}</span>=
                    <span className="font-mono">{seg.value}</span>
                  </span>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
