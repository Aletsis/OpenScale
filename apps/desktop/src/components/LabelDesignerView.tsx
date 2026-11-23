import {
  ABML_FONTS,
  LABEL_FIELD_TYPES,
  LABEL_MAX_SLOTS,
  type LabelFieldModel,
  type LabelFormatModel,
  type Plu,
  type Scale,
  createBlankLabelFormat,
  findNextFreeSlot,
  getAbmlFontSpec,
  hundredthsToMm,
  mmToHundredths,
  mmToTenths,
  resolveFieldPreviewText,
  rotationDegrees,
  tenthsToMm,
  validateLabelFormat,
} from "@openscale/client";
import { Badge, Button } from "@openscale/ui";
import { invoke } from "@tauri-apps/api/core";
import {
  CheckCircle2,
  Download,
  FilePlus2,
  Printer,
  RotateCw,
  Send,
  Trash2,
  Upload,
} from "lucide-react";
import { useMemo, useRef, useState } from "react";

const PX_PER_MM = 5;

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

export function LabelDesignerView({ scales, plus, onStatus }: LabelDesignerViewProps) {
  const xsScales = scales.filter((s) => s.family === "avery_berkel_xs");
  const [host, setHost] = useState<string>(xsScales[0]?.ip_address ?? "192.168.9.113");
  const [label, setLabel] = useState<LabelFormatModel>(() => createBlankLabelFormat(98, 59));
  const [heightMm, setHeightMm] = useState<number>(40);
  const [selectedSlot, setSelectedSlot] = useState<number | null>(null);
  const [previewPluId, setPreviewPluId] = useState<string>(plus[0]?.id ?? "");
  const [sampleWeightKg, setSampleWeightKg] = useState<number>(0.5);
  const dragRef = useRef<DragState | null>(null);

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

  const updateField = (slot: number, patch: Partial<LabelFieldModel>) => {
    setLabel((prev) => ({
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
    const meta = LABEL_FIELD_TYPES.find((t) => t.ft === ft);
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
    setLabel((prev) => ({ ...prev, active: true, fields: [...prev.fields, newField] }));
    setSelectedSlot(nextSlot);
  };

  const handleDeleteField = (slot: number) => {
    setLabel((prev) => ({
      ...prev,
      fields: prev.fields.filter((f) => f.slot !== slot),
    }));
    if (selectedSlot === slot) {
      setSelectedSlot(null);
    }
  };

  const handleLoadFromScale = async () => {
    try {
      const rec = await invoke<LabelFormatModel>("xs_read_label_format", {
        host,
        port: null,
        pfid: label.pfid,
      });
      setLabel(rec);
      setSelectedSlot(rec.fields[0]?.slot ?? null);
      const maxY = rec.fields.reduce((acc, f) => Math.max(acc, tenthsToMm(f.y2)), 40);
      setHeightMm(Math.max(30, Math.ceil(maxY + 4)));
      onStatus(`Loaded PFID ${rec.pfid} from ${host} (${rec.fields.length} fields).`);
    } catch (err) {
      onStatus(`Load PFID error: ${String(err)}`);
    }
  };

  const handleSendToScale = async () => {
    if (!validation.valid) {
      onStatus(`Cannot send label: ${validation.errors[0]}`);
      return;
    }
    try {
      await invoke("xs_write_label_format", {
        host,
        port: null,
        record: label,
      });
      onStatus(`Sent PFID ${label.pfid} (898 B) to ${host}.`);
    } catch (err) {
      onStatus(`Send PFID error: ${String(err)}`);
    }
  };

  const handleTestPrint = async () => {
    try {
      const res = await invoke<string>("xs_service_action", {
        host,
        port: null,
        action: "print_test",
      });
      onStatus(
        res
          ? `Test print command sent to ${host} (${res}).`
          : `Test print command sent to ${host}.`,
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

  return (
    <div className="space-y-4">
      {/* Top Toolbar */}
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-card p-3 text-sm">
        <div className="flex flex-wrap items-center gap-2">
          <input
            type="text"
            value={host}
            onChange={(ev) => setHost(ev.target.value)}
            placeholder="XS IP"
            className="w-36 rounded border border-border bg-background px-2 py-1"
          />
          <label className="flex items-center gap-1">
            <span className="text-xs text-muted-foreground">PFID</span>
            <input
              type="number"
              min={0}
              max={99}
              value={label.pfid}
              onChange={(ev) =>
                setLabel((prev) => ({
                  ...prev,
                  pfid: Math.max(0, Math.min(99, Number.parseInt(ev.target.value, 10) || 0)),
                }))
              }
              className="w-16 rounded border border-border bg-background px-2 py-1"
            />
          </label>
          <Button size="sm" variant="outline" onClick={() => void handleLoadFromScale()}>
            <Upload className="h-3.5 w-3.5" /> Load
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              setLabel(createBlankLabelFormat(label.pfid, 59));
              setHeightMm(40);
              setSelectedSlot(null);
            }}
          >
            <FilePlus2 className="h-3.5 w-3.5" /> Blank 59×40
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Badge variant={validation.valid ? "success" : "danger"}>
            <CheckCircle2 className="mr-1 h-3 w-3" />
            {validation.byteSize} B • {label.fields.length}/{LABEL_MAX_SLOTS}
          </Badge>
          <Button size="sm" variant="outline" onClick={handleExportJson}>
            <Download className="h-3.5 w-3.5" /> JSON
          </Button>
          <Button size="sm" disabled={!validation.valid} onClick={() => void handleSendToScale()}>
            <Send className="h-3.5 w-3.5" /> Send to Scale
          </Button>
          <Button
            size="sm"
            variant="secondary"
            title="Print scale diagnostic test report (CMD 47)"
            onClick={() => void handleTestPrint()}
          >
            <Printer className="h-3.5 w-3.5" /> Test Print
          </Button>
        </div>
      </div>

      {/* Live PLU Preview Selector */}
      <div className="flex flex-wrap items-center gap-3 rounded-md border border-border bg-card px-3 py-2 text-xs">
        <span className="font-medium text-muted-foreground">Live PLU Preview:</span>
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
          <span className="text-muted-foreground">Weight (kg):</span>
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
            Add Field (FT)
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
              Label Format
            </h3>
            <label className="flex flex-col gap-1">
              <span>Name (max 27 chars)</span>
              <input
                type="text"
                maxLength={27}
                value={label.name}
                onChange={(ev) => setLabel((prev) => ({ ...prev, name: ev.target.value }))}
                className="rounded border border-border bg-background px-2 py-1"
              />
            </label>
            <div className="grid grid-cols-2 gap-2">
              <label className="flex flex-col gap-1">
                <span>Width (mm)</span>
                <input
                  type="number"
                  min={10}
                  max={100}
                  value={widthMm}
                  onChange={(ev) =>
                    setLabel((prev) => ({
                      ...prev,
                      width_hundredths_mm: mmToHundredths(Number(ev.target.value) || 59),
                    }))
                  }
                  className="rounded border border-border bg-background px-2 py-1"
                />
              </label>
              <label className="flex flex-col gap-1">
                <span>Height (mm)</span>
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
          </div>

          {selectedField ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <h3 className="font-semibold uppercase tracking-wide text-muted-foreground">
                  Field Slot #{selectedField.slot} (FT {selectedField.field_type})
                </h3>
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
            <p className="text-muted-foreground">
              Click any field on the canvas to edit its mm coordinates, ABML font, rotation, or DSR
              reference.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
