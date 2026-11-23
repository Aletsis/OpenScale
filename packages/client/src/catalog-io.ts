import * as XLSX from "xlsx";

import type { BulkPriceAdjustment, Plu, ProductType } from "./types.js";

const CSV_HEADERS = [
  "did",
  "plu",
  "description",
  "price_1",
  "price_2",
  "product_type",
  "ean",
  "tare_grams",
  "date_offset_1",
] as const;

const VALID_PRODUCT_TYPES: ReadonlySet<string> = new Set<ProductType>([
  "by_weight",
  "by_count",
  "negative_by_count",
  "fixed_weight",
  "fixed_price",
  "manual_weight",
  "by_volume",
]);

function escapeCsvCell(value: string): string {
  if (/[",\r\n]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function parseCsvLine(line: string): string[] {
  const cells: string[] = [];
  let current = "";
  let inQuotes = false;

  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (inQuotes) {
      if (ch === '"') {
        if (line[i + 1] === '"') {
          current += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        current += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === ",") {
      cells.push(current.trim());
      current = "";
    } else {
      current += ch;
    }
  }
  cells.push(current.trim());
  return cells;
}

function parsePriceToCents(raw: string | number | undefined): number {
  if (typeof raw === "number") {
    return Number.isFinite(raw) ? Math.max(0, Math.round(raw * 100)) : 0;
  }
  const cleaned = String(raw ?? "")
    .replace(/[$,\s]/g, "")
    .trim();
  if (!cleaned) {
    return 0;
  }
  const num = Number.parseFloat(cleaned);
  return Number.isFinite(num) ? Math.max(0, Math.round(num * 100)) : 0;
}

function buildPluFromRow(
  row: Record<string, string | number | undefined>,
  storeId: string,
): Plu | null {
  const did = Number.parseInt(String(row.did ?? "1"), 10);
  const pluNum = Number.parseInt(String(row.plu ?? ""), 10);
  if (!Number.isFinite(did) || did < 0 || !Number.isFinite(pluNum) || pluNum <= 0) {
    return null;
  }

  const rawType = String(row.product_type ?? "by_weight").trim();
  const productType: ProductType = VALID_PRODUCT_TYPES.has(rawType)
    ? (rawType as ProductType)
    : "by_weight";

  const tareRaw = String(row.tare_grams ?? "").trim();
  const tareGrams = tareRaw ? Number.parseInt(tareRaw, 10) : null;
  const dateOffsetRaw = Number.parseInt(String(row.date_offset_1 ?? "0"), 10);
  const now = new Date().toISOString();

  return {
    id: crypto.randomUUID(),
    key: { did, plu: pluNum },
    description: String(row.description ?? "").trim(),
    button_text: null,
    price_1: parsePriceToCents(row.price_1),
    price_2: parsePriceToCents(row.price_2),
    price_3: 0,
    product_type: productType,
    ean: String(row.ean ?? "").trim() || null,
    sku: null,
    group_id: 0,
    barcode_format_id: 1,
    label_format_id: 1,
    tare_id: null,
    tare_grams: tareGrams !== null && Number.isFinite(tareGrams) ? Math.max(0, tareGrams) : null,
    date_offset_1: Number.isFinite(dateOffsetRaw) ? Math.max(0, dateOffsetRaw) : 0,
    date_print_control_1: dateOffsetRaw > 0 ? 1 : 0,
    date_offset_2: 0,
    date_print_control_2: 0,
    tax_rate_id_1: 0,
    tax_rate_id_2: 0,
    text_areas: [],
    delta_hash: "",
    meta: {
      store_id: storeId,
      sync_version: 1,
      updated_at: now,
      deleted_at: null,
    },
  };
}

/**
 * Serializes a list of PLUs into RFC-4180 CSV with decimal prices (e.g. 45.00).
 */
export function serializePlusToCsv(plus: Plu[]): string {
  const lines: string[] = [CSV_HEADERS.join(",")];
  for (const plu of plus) {
    const row = [
      String(plu.key.did),
      String(plu.key.plu),
      escapeCsvCell(plu.description),
      (plu.price_1 / 100).toFixed(2),
      (plu.price_2 / 100).toFixed(2),
      plu.product_type,
      escapeCsvCell(plu.ean ?? ""),
      plu.tare_grams !== null ? String(plu.tare_grams) : "",
      String(plu.date_offset_1),
    ];
    lines.push(row.join(","));
  }
  return lines.join("\n");
}

/**
 * Parses an RFC-4180 CSV string into domain `Plu` objects.
 */
export function parsePlusFromCsv(csv: string, storeId: string): Plu[] {
  const rawLines = csv
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (rawLines.length < 2) {
    return [];
  }

  const headers = parseCsvLine(rawLines[0] ?? "").map((h) => h.toLowerCase());
  const results: Plu[] = [];

  for (let i = 1; i < rawLines.length; i++) {
    const values = parseCsvLine(rawLines[i] ?? "");
    const record: Record<string, string> = {};
    for (let j = 0; j < headers.length; j++) {
      const key = headers[j];
      if (key) {
        record[key] = values[j] ?? "";
      }
    }
    const plu = buildPluFromRow(record, storeId);
    if (plu) {
      results.push(plu);
    }
  }
  return results;
}

/**
 * Exports PLUs to an Excel `.xlsx` binary buffer.
 */
export function exportPlusToXlsxBuffer(plus: Plu[]): Uint8Array {
  const rows = plus.map((plu) => ({
    did: plu.key.did,
    plu: plu.key.plu,
    description: plu.description,
    price_1: Number((plu.price_1 / 100).toFixed(2)),
    price_2: Number((plu.price_2 / 100).toFixed(2)),
    product_type: plu.product_type,
    ean: plu.ean ?? "",
    tare_grams: plu.tare_grams ?? "",
    date_offset_1: plu.date_offset_1,
  }));

  const sheet = XLSX.utils.json_to_sheet(rows, { header: [...CSV_HEADERS] });
  const workbook = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(workbook, sheet, "PLUs");
  const out = XLSX.write(workbook, { type: "array", bookType: "xlsx" }) as ArrayBuffer;
  return new Uint8Array(out);
}

/**
 * Parses PLUs from an Excel `.xlsx` binary buffer.
 */
export function parsePlusFromXlsxBuffer(buffer: ArrayBuffer | Uint8Array, storeId: string): Plu[] {
  const workbook = XLSX.read(buffer, { type: "array" });
  const firstSheetName = workbook.SheetNames[0];
  if (!firstSheetName) {
    return [];
  }
  const sheet = workbook.Sheets[firstSheetName];
  if (!sheet) {
    return [];
  }
  const rows = XLSX.utils.sheet_to_json<Record<string, string | number | undefined>>(sheet);
  const results: Plu[] = [];
  for (const rawRow of rows) {
    const normalized: Record<string, string | number | undefined> = {};
    for (const [k, v] of Object.entries(rawRow)) {
      normalized[k.toLowerCase().trim()] = v;
    }
    const plu = buildPluFromRow(normalized, storeId);
    if (plu) {
      results.push(plu);
    }
  }
  return results;
}

/**
 * Applies a percentage or fixed-cents price adjustment across a batch of PLUs (clamped to >= 0).
 */
export function applyBulkPriceAdjustment(plus: Plu[], adjustment: BulkPriceAdjustment): Plu[] {
  const now = new Date().toISOString();
  return plus.map((plu) => {
    const nextPrice =
      adjustment.mode === "percent"
        ? Math.round(plu.price_1 * (1 + adjustment.value / 100))
        : plu.price_1 + Math.round(adjustment.value);

    return {
      ...plu,
      price_1: Math.max(0, nextPrice),
      delta_hash: "",
      meta: {
        ...plu.meta,
        updated_at: now,
      },
    };
  });
}
