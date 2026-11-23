import {
  type PluTotals,
  type SalesSnapshot,
  type Scale,
  type ScaleSalesReport,
  buildPrintableSalesReportHtml,
  exportSalesReportToXlsxBuffer,
  serializeSalesReportToCsv,
} from "@openscale/client";
import { Badge, Button, DataTable } from "@openscale/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { invoke } from "@tauri-apps/api/core";
import { BarChart3, Download, Printer } from "lucide-react";
import { useMemo, useState } from "react";

interface SalesReportsViewProps {
  scales: Scale[];
  onStatus: (msg: string) => void;
}

function downloadBlob(data: BlobPart, filename: string, mimeType: string) {
  const blob = new Blob([data], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export function SalesReportsView({ scales, onStatus }: SalesReportsViewProps) {
  const queryClient = useQueryClient();
  const [selectedScaleId, setSelectedScaleId] = useState<string>(scales[0]?.id ?? "");
  const [period, setPeriod] = useState<number>(0);
  const [activeReport, setActiveReport] = useState<ScaleSalesReport | null>(null);

  const snapshotsQuery = useQuery({
    queryKey: ["sales_snapshots", selectedScaleId],
    queryFn: () =>
      invoke<SalesSnapshot[]>("list_sales_snapshots", {
        scaleId: selectedScaleId || null,
      }),
  });

  const captureMutation = useMutation({
    mutationFn: () =>
      invoke<ScaleSalesReport>("capture_sales_snapshot", {
        scaleId: selectedScaleId,
        period,
      }),
    onSuccess: async (report) => {
      setActiveReport(report);
      await queryClient.invalidateQueries({ queryKey: ["sales_snapshots"] });
      onStatus(
        `Captured sales snapshot (${report.plu_totals.length} PLUs, period ${report.period}).`,
      );
    },
    onError: (err) => onStatus(`Sales capture error: ${String(err)}`),
  });

  const topPluBars = useMemo(() => {
    if (!activeReport) {
      return [];
    }
    const sorted = [...activeReport.plu_totals]
      .sort((a, b) => b.counter_sales + b.prepack_sales - (a.counter_sales + a.prepack_sales))
      .slice(0, 8);
    const maxVal = Math.max(1, ...sorted.map((p) => p.counter_sales + p.prepack_sales));
    return sorted.map((p) => ({
      label: `${p.key.did}:${p.key.plu}`,
      totalCents: p.counter_sales + p.prepack_sales,
      pct: Math.round(((p.counter_sales + p.prepack_sales) / maxVal) * 100),
    }));
  }, [activeReport]);

  const pluTotalCols: ColumnDef<PluTotals>[] = [
    { id: "key", header: "DID:PLU", accessorFn: (r) => `${r.key.did}:${r.key.plu}` },
    { accessorKey: "transactions", header: "Tx" },
    {
      id: "counter",
      header: "Counter",
      accessorFn: (r) => `$${(r.counter_sales / 100).toFixed(2)}`,
    },
    {
      id: "prepack",
      header: "Pre-Pack",
      accessorFn: (r) => `$${(r.prepack_sales / 100).toFixed(2)}`,
    },
    { accessorKey: "counter_quantity", header: "Qty (g/pcs)" },
  ];

  const scaleName = scales.find((s) => s.id === selectedScaleId)?.name ?? "Scale";

  const handlePrintPdf = () => {
    if (!activeReport) {
      return;
    }
    const html = buildPrintableSalesReportHtml(activeReport, scaleName);
    const win = window.open("", "_blank");
    if (win) {
      win.document.write(html);
      win.document.close();
      win.print();
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <select
            value={selectedScaleId}
            onChange={(ev) => setSelectedScaleId(ev.target.value)}
            className="rounded-md border border-border bg-card px-2 py-1.5 text-sm"
          >
            <option value="">Select scale...</option>
            {scales.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name} ({s.ip_address})
              </option>
            ))}
          </select>
          <select
            value={period}
            onChange={(ev) => setPeriod(Number(ev.target.value))}
            className="rounded-md border border-border bg-card px-2 py-1.5 text-sm"
          >
            <option value={0}>Period 0 (Current)</option>
            <option value={1}>Period 1 (Previous)</option>
          </select>
          <Button
            size="sm"
            disabled={!selectedScaleId || captureMutation.isPending}
            onClick={() => captureMutation.mutate()}
          >
            <BarChart3 className="h-4 w-4" />
            Capture Snapshot
          </Button>
        </div>

        {activeReport && (
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                downloadBlob(
                  serializeSalesReportToCsv(activeReport),
                  "openscale-sales.csv",
                  "text/csv;charset=utf-8",
                )
              }
            >
              <Download className="h-4 w-4" />
              CSV
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                downloadBlob(
                  exportSalesReportToXlsxBuffer(activeReport).buffer as ArrayBuffer,
                  "openscale-sales.xlsx",
                  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                )
              }
            >
              <Download className="h-4 w-4" />
              Excel
            </Button>
            <Button size="sm" variant="outline" onClick={handlePrintPdf}>
              <Printer className="h-4 w-4" />
              PDF
            </Button>
          </div>
        )}
      </div>

      {activeReport && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <div className="rounded-md border border-border bg-card p-3">
              <p className="text-xs text-muted-foreground">Gross Total</p>
              <p className="text-lg font-bold">
                ${((activeReport.grand_totals?.gross_total ?? 0) / 100).toFixed(2)}
              </p>
            </div>
            <div className="rounded-md border border-border bg-card p-3">
              <p className="text-xs text-muted-foreground">Counter Sales</p>
              <p className="text-lg font-bold">
                ${((activeReport.grand_totals?.counter_sales ?? 0) / 100).toFixed(2)}
              </p>
            </div>
            <div className="rounded-md border border-border bg-card p-3">
              <p className="text-xs text-muted-foreground">Pre-Pack Sales</p>
              <p className="text-lg font-bold">
                ${((activeReport.grand_totals?.prepack_sales ?? 0) / 100).toFixed(2)}
              </p>
            </div>
            <div className="rounded-md border border-border bg-card p-3">
              <p className="text-xs text-muted-foreground">Transactions</p>
              <p className="text-lg font-bold">{activeReport.grand_totals?.transactions ?? 0}</p>
            </div>
          </div>

          {topPluBars.length > 0 && (
            <div className="space-y-2 rounded-md border border-border bg-card p-4">
              <h3 className="text-sm font-medium">Top PLUs by Sales</h3>
              <div className="space-y-2">
                {topPluBars.map((bar) => (
                  <div key={bar.label} className="flex items-center gap-3 text-xs">
                    <span className="w-16 font-mono">{bar.label}</span>
                    <div className="h-4 flex-1 overflow-hidden rounded bg-muted">
                      <div
                        className="h-full bg-primary"
                        style={{ width: `${Math.max(2, bar.pct)}%` }}
                      />
                    </div>
                    <span className="w-20 text-right font-medium">
                      ${(bar.totalCents / 100).toFixed(2)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          <DataTable columns={pluTotalCols} data={activeReport.plu_totals} />
        </>
      )}

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-muted-foreground">Snapshot History (SQLite)</h3>
        <div className="flex flex-wrap gap-2">
          {(snapshotsQuery.data ?? []).map((snap) => (
            <Button
              key={snap.id}
              size="sm"
              variant="outline"
              onClick={() => {
                try {
                  setActiveReport(JSON.parse(snap.payload_json) as ScaleSalesReport);
                } catch {
                  onStatus("Could not parse snapshot payload.");
                }
              }}
            >
              <Badge variant="default">P{snap.period}</Badge>
              <span>{new Date(snap.captured_at).toLocaleString()}</span>
            </Button>
          ))}
        </div>
      </section>
    </div>
  );
}
