import { describe, expect, it } from "vitest";
import * as XLSX from "xlsx";

import {
  buildPrintableSalesReportHtml,
  exportSalesReportToXlsxBuffer,
  serializeSalesReportToCsv,
} from "./sales-export.js";
import type { ScaleSalesReport } from "./types.js";

const SAMPLE_REPORT: ScaleSalesReport = {
  scale_id: "scale-1",
  period: 0,
  captured_at: "2026-11-22T20:00:00Z",
  grand_totals: {
    period: 0,
    counter_sales: 130480,
    prepack_sales: 25000,
    transactions: 40,
    gross_total: 155480,
    last_cleared_at: "2026-11-22T00:00:00",
  },
  dept_totals: [
    {
      period: 0,
      did: 1,
      counter_sales: 130480,
      prepack_sales: 25000,
      transactions: 40,
      last_cleared_at: null,
    },
  ],
  plu_totals: [
    {
      period: 0,
      key: { did: 1, plu: 1 },
      transactions: 34,
      counter_sales: 130480,
      counter_quantity: 18640,
      counter_transactions: 34,
      prepack_sales: 0,
      prepack_quantity: 0,
      last_cleared_at: null,
    },
  ],
};

describe("sales-export (CSV, XLSX, printable PDF HTML)", () => {
  it("serializes PLU sales totals to CSV", () => {
    const csv = serializeSalesReportToCsv(SAMPLE_REPORT);
    expect(csv).toContain("did,plu,transactions,counter_sales");
    expect(csv).toContain("1,1,34,1304.80,18640,0.00,0");
  });

  it("exports multi-sheet XLSX workbook with Summary, Departments, and PLU_Totals", () => {
    const buf = exportSalesReportToXlsxBuffer(SAMPLE_REPORT);
    const wb = XLSX.read(buf, { type: "array" });
    expect(wb.SheetNames).toEqual(["Summary", "Departments", "PLU_Totals"]);
  });

  it("builds printable HTML for PDF export", () => {
    const html = buildPrintableSalesReportHtml(SAMPLE_REPORT, "XS200 Carnicería");
    expect(html).toContain("XS200 Carnicería");
    expect(html).toContain("$1554.80");
    expect(html).toContain("$1304.80");
  });
});
