import * as XLSX from "xlsx";

import type { ScaleSalesReport } from "./types.js";

/**
 * Serializes PLU-level sales totals from a `ScaleSalesReport` into CSV.
 */
export function serializeSalesReportToCsv(report: ScaleSalesReport): string {
  const headers = [
    "did",
    "plu",
    "transactions",
    "counter_sales",
    "counter_quantity",
    "prepack_sales",
    "prepack_quantity",
  ];
  const lines = [headers.join(",")];
  for (const row of report.plu_totals) {
    lines.push(
      [
        row.key.did,
        row.key.plu,
        row.transactions,
        (row.counter_sales / 100).toFixed(2),
        row.counter_quantity,
        (row.prepack_sales / 100).toFixed(2),
        row.prepack_quantity,
      ].join(","),
    );
  }
  return lines.join("\n");
}

/**
 * Exports a `ScaleSalesReport` into a multi-sheet Excel (`.xlsx`) workbook
 * with Summary, Departments, and PLU Totals sheets.
 */
export function exportSalesReportToXlsxBuffer(report: ScaleSalesReport): Uint8Array {
  const workbook = XLSX.utils.book_new();

  const summaryRows = [
    {
      scale_id: report.scale_id,
      period: report.period,
      captured_at: report.captured_at,
      counter_sales: ((report.grand_totals?.counter_sales ?? 0) / 100).toFixed(2),
      prepack_sales: ((report.grand_totals?.prepack_sales ?? 0) / 100).toFixed(2),
      gross_total: ((report.grand_totals?.gross_total ?? 0) / 100).toFixed(2),
      transactions: report.grand_totals?.transactions ?? 0,
    },
  ];
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.json_to_sheet(summaryRows), "Summary");

  const deptRows = report.dept_totals.map((d) => ({
    did: d.did,
    counter_sales: Number((d.counter_sales / 100).toFixed(2)),
    prepack_sales: Number((d.prepack_sales / 100).toFixed(2)),
    transactions: d.transactions,
  }));
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.json_to_sheet(deptRows), "Departments");

  const pluRows = report.plu_totals.map((p) => ({
    did: p.key.did,
    plu: p.key.plu,
    transactions: p.transactions,
    counter_sales: Number((p.counter_sales / 100).toFixed(2)),
    counter_quantity: p.counter_quantity,
    prepack_sales: Number((p.prepack_sales / 100).toFixed(2)),
    prepack_quantity: p.prepack_quantity,
  }));
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.json_to_sheet(pluRows), "PLU_Totals");

  const out = XLSX.write(workbook, { type: "array", bookType: "xlsx" }) as ArrayBuffer;
  return new Uint8Array(out);
}

/**
 * Generates a self-contained printable HTML document suitable for Save-as-PDF (`window.print()`).
 */
export function buildPrintableSalesReportHtml(report: ScaleSalesReport, scaleName: string): string {
  const gross = ((report.grand_totals?.gross_total ?? 0) / 100).toFixed(2);
  const tx = report.grand_totals?.transactions ?? 0;
  const rows = report.plu_totals
    .map(
      (p) =>
        `<tr><td>${p.key.did}</td><td>${p.key.plu}</td><td>${p.transactions}</td><td>$${(p.counter_sales / 100).toFixed(2)}</td><td>$${(p.prepack_sales / 100).toFixed(2)}</td></tr>`,
    )
    .join("");

  return `<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<title>OpenScale Sales Report - ${scaleName}</title>
<style>
  body { font-family: system-ui, sans-serif; padding: 24px; color: #0f172a; }
  h1 { margin: 0 0 8px; }
  table { width: 100%; border-collapse: collapse; margin-top: 16px; }
  th, td { border: 1px solid #cbd5e1; padding: 6px 8px; text-align: left; font-size: 13px; }
  th { background: #f1f5f9; }
</style>
</head>
<body>
  <h1>OpenScale Sales Report — ${scaleName}</h1>
  <p>Period: ${report.period} | Captured: ${report.captured_at} | Gross Total: $${gross} | Transactions: ${tx}</p>
  <table>
    <thead><tr><th>Dept</th><th>PLU</th><th>Tx</th><th>Counter</th><th>Pre-Pack</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>
</body>
</html>`;
}
