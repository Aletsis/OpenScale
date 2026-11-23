import type { Scale } from "@openscale/client";
import { Badge, Button, DataTable } from "@openscale/ui";
import type { ColumnDef } from "@tanstack/react-table";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Download, FileCode2 } from "lucide-react";
import { useState } from "react";

interface LabelFieldDto {
  slot: number;
  field_type: number;
  orientation: number;
  style: number;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  font: string;
  dsr: number;
}

interface LabelFormatDto {
  pfid: number;
  orientation: number;
  active: boolean;
  width_hundredths_mm: number;
  name: string;
  fields: LabelFieldDto[];
  raw: number[];
}

interface LabelFormatsViewProps {
  scales: Scale[];
  onStatus: (msg: string) => void;
}

export function LabelFormatsView({ scales, onStatus }: LabelFormatsViewProps) {
  const xsScales = scales.filter((s) => s.family === "avery_berkel_xs");
  const [host, setHost] = useState<string>(xsScales[0]?.ip_address ?? "192.168.9.113");
  const [pfid, setPfid] = useState("1");
  const [targetHost, setTargetHost] = useState<string>(xsScales[0]?.ip_address ?? "192.168.9.113");
  const [targetPfid, setTargetPfid] = useState("98");
  const [label, setLabel] = useState<LabelFormatDto | null>(null);

  const handleRead = async () => {
    try {
      const rec = await invoke<LabelFormatDto>("xs_read_label_format", {
        host,
        port: null,
        pfid: Number.parseInt(pfid, 10) || 0,
      });
      setLabel(rec);
      onStatus(`Read PFID ${rec.pfid} (${rec.fields.length} active fields, 898 B).`);
    } catch (err) {
      onStatus(`Read label error: ${String(err)}`);
    }
  };

  const handleClone = async () => {
    if (!label) {
      return;
    }
    try {
      const cloned: LabelFormatDto = {
        ...label,
        pfid: Number.parseInt(targetPfid, 10) || label.pfid,
      };
      await invoke("xs_write_label_format", {
        host: targetHost,
        port: null,
        record: cloned,
      });
      onStatus(`Cloned PFID ${label.pfid} -> ${targetHost} PFID ${cloned.pfid}.`);
    } catch (err) {
      onStatus(`Clone label error: ${String(err)}`);
    }
  };

  const handleBackupJson = () => {
    if (!label) {
      return;
    }
    const blob = new Blob([JSON.stringify(label, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `label-pfid-${label.pfid}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const fieldCols: ColumnDef<LabelFieldDto>[] = [
    { accessorKey: "slot", header: "Slot" },
    { accessorKey: "field_type", header: "FT" },
    {
      id: "coords",
      header: "X1,Y1 → X2,Y2 (0.1mm)",
      accessorFn: (f) => `${f.x1},${f.y1} → ${f.x2},${f.y2}`,
    },
    { accessorKey: "orientation", header: "Rot" },
    { accessorKey: "font", header: "Font" },
    { accessorKey: "dsr", header: "DSR" },
  ];

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-card p-3 text-sm">
        <FileCode2 className="h-4 w-4 text-primary" />
        <input
          type="text"
          value={host}
          onChange={(ev) => setHost(ev.target.value)}
          placeholder="Source XS IP"
          className="w-40 rounded border border-border bg-background px-2 py-1"
        />
        <input
          type="number"
          min={0}
          max={99}
          value={pfid}
          onChange={(ev) => setPfid(ev.target.value)}
          className="w-20 rounded border border-border bg-background px-2 py-1"
          placeholder="PFID"
        />
        <Button size="sm" onClick={() => void handleRead()}>
          Read PFID (898 B)
        </Button>
        {label && (
          <Button size="sm" variant="outline" onClick={handleBackupJson}>
            <Download className="h-3.5 w-3.5" /> Backup JSON
          </Button>
        )}
      </div>

      {label && (
        <>
          <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-card p-3 text-sm">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={label.active ? "success" : "default"}>
                PFID {label.pfid} ({label.active ? "Active" : "Empty"})
              </Badge>
              <span>Name: {label.name || "—"}</span>
              <span>Width: {(label.width_hundredths_mm / 100).toFixed(1)} mm</span>
              <span>Fields: {label.fields.length} / 50</span>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={targetHost}
                onChange={(ev) => setTargetHost(ev.target.value)}
                placeholder="Target XS IP"
                className="w-36 rounded border border-border bg-background px-2 py-1"
              />
              <input
                type="number"
                min={0}
                max={99}
                value={targetPfid}
                onChange={(ev) => setTargetPfid(ev.target.value)}
                className="w-16 rounded border border-border bg-background px-2 py-1"
              />
              <Button size="sm" variant="secondary" onClick={() => void handleClone()}>
                <Copy className="h-3.5 w-3.5" /> Clone
              </Button>
            </div>
          </div>

          <DataTable columns={fieldCols} data={label.fields} />
        </>
      )}
    </div>
  );
}
