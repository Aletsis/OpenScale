import { describe, expect, it } from "vitest";

import {
  applyBulkPriceAdjustment,
  exportPlusToXlsxBuffer,
  parsePlusFromCsv,
  parsePlusFromXlsxBuffer,
  serializePlusToCsv,
} from "./catalog-io.js";
import type { Plu } from "./types.js";

const STORE_ID = "01900000-0000-7000-8000-000000000001";

function makeSamplePlu(overrides: Partial<Plu> = {}): Plu {
  return {
    id: "plu-1",
    key: { did: 1, plu: 100 },
    description: 'POLLO "FRESCO", ENTERO',
    button_text: null,
    price_1: 7000,
    price_2: 6500,
    price_3: 0,
    product_type: "by_weight",
    ean: "2101001000000",
    sku: null,
    group_id: 0,
    barcode_format_id: 1,
    label_format_id: 1,
    tare_id: null,
    tare_grams: 15,
    date_offset_1: 7,
    date_print_control_1: 1,
    date_offset_2: 0,
    date_print_control_2: 0,
    tax_rate_id_1: 0,
    tax_rate_id_2: 0,
    text_areas: [],
    delta_hash: "h1",
    meta: {
      store_id: STORE_ID,
      sync_version: 1,
      updated_at: "2026-11-22T00:00:00Z",
      deleted_at: null,
    },
    ...overrides,
  };
}

describe("catalog-io (CSV, XLSX, bulk price)", () => {
  it("roundtrips PLUs through RFC-4180 CSV with quotes and commas", () => {
    const original = [
      makeSamplePlu(),
      makeSamplePlu({
        id: "plu-2",
        key: { did: 2, plu: 250 },
        description: "QUESO MANCHEGO",
        price_1: 14550,
        price_2: 0,
        product_type: "by_count",
        ean: null,
        tare_grams: null,
        date_offset_1: 0,
      }),
    ];

    const csv = serializePlusToCsv(original);
    const parsed = parsePlusFromCsv(csv, STORE_ID);

    expect(parsed).toHaveLength(2);
    expect(parsed[0]?.key).toEqual({ did: 1, plu: 100 });
    expect(parsed[0]?.description).toBe('POLLO "FRESCO", ENTERO');
    expect(parsed[0]?.price_1).toBe(7000);
    expect(parsed[0]?.price_2).toBe(6500);
    expect(parsed[0]?.tare_grams).toBe(15);
    expect(parsed[0]?.date_offset_1).toBe(7);

    expect(parsed[1]?.key).toEqual({ did: 2, plu: 250 });
    expect(parsed[1]?.price_1).toBe(14550);
    expect(parsed[1]?.product_type).toBe("by_count");
    expect(parsed[1]?.ean).toBeNull();
    expect(parsed[1]?.tare_grams).toBeNull();
  });

  it("ignores invalid rows and empty CSV inputs", () => {
    expect(parsePlusFromCsv("", STORE_ID)).toEqual([]);
    expect(parsePlusFromCsv("did,plu,description\n1,0,BAD\n-1,5,BAD2", STORE_ID)).toEqual([]);
  });

  it("roundtrips PLUs through Excel (.xlsx) binary buffers", () => {
    const original = [makeSamplePlu({ description: "JAMÓN DE PAVO" })];
    const buffer = exportPlusToXlsxBuffer(original);
    expect(buffer.byteLength).toBeGreaterThan(100);

    const parsed = parsePlusFromXlsxBuffer(buffer, STORE_ID);
    expect(parsed).toHaveLength(1);
    expect(parsed[0]?.key).toEqual({ did: 1, plu: 100 });
    expect(parsed[0]?.description).toBe("JAMÓN DE PAVO");
    expect(parsed[0]?.price_1).toBe(7000);
    expect(parsed[0]?.ean).toBe("2101001000000");
  });

  it("applies percentage and fixed-cents bulk price adjustments clamped at zero", () => {
    const items = [makeSamplePlu({ price_1: 10000 }), makeSamplePlu({ price_1: 500 })];

    const plus10Pct = applyBulkPriceAdjustment(items, { mode: "percent", value: 10 });
    expect(plus10Pct[0]?.price_1).toBe(11000);
    expect(plus10Pct[1]?.price_1).toBe(550);

    const minus1000 = applyBulkPriceAdjustment(items, { mode: "fixed_cents", value: -1000 });
    expect(minus1000[0]?.price_1).toBe(9000);
    expect(minus1000[1]?.price_1).toBe(0);
  });
});
