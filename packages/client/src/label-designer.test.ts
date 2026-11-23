import { describe, expect, it } from "vitest";

import {
  LABEL_FORMAT_BYTE_LIMIT,
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
} from "./label-designer.js";
import type { Plu } from "./types.js";

const SAMPLE_PLU: Plu = {
  id: "plu-1",
  key: { did: 1, plu: 1 },
  description: "ALITA DE POLLO ADOBADA",
  button_text: null,
  price_1: 7000,
  price_2: 0,
  price_3: 0,
  product_type: "by_weight",
  ean: "2101001000000",
  sku: null,
  group_id: 1,
  barcode_format_id: 1,
  label_format_id: 1,
  tare_id: 0,
  tare_grams: 20,
  date_offset_1: 5,
  date_print_control_1: 1,
  date_offset_2: 0,
  date_print_control_2: 0,
  tax_rate_id_1: 0,
  tax_rate_id_2: 0,
  text_areas: ["Pollo marinado", "Mantener a 4C"],
  delta_hash: "h",
  meta: {
    store_id: "store-1",
    sync_version: 1,
    updated_at: "2026-11-23T00:00:00Z",
    deleted_at: null,
  },
};

describe("label-designer engine", () => {
  it("converts mm to tenths/hundredths and rotations accurately", () => {
    expect(mmToTenths(59.4)).toBe(594);
    expect(tenthsToMm(594)).toBe(59.4);
    expect(mmToHundredths(59)).toBe(5900);
    expect(hundredthsToMm(5900)).toBe(59);
    expect(rotationDegrees(0)).toBe(0);
    expect(rotationDegrees(1)).toBe(90);
    expect(rotationDegrees(2)).toBe(180);
    expect(rotationDegrees(3)).toBe(270);
    expect(getAbmlFontSpec("E").pxSize).toBe(22);
    expect(getAbmlFontSpec("Z").pxSize).toBe(12);
  });

  it("validates 898-byte XS limits, slot bounds, and coordinate invariants", () => {
    const label = createBlankLabelFormat(98, 59);
    expect(label.raw.length).toBe(LABEL_FORMAT_BYTE_LIMIT);
    expect(validateLabelFormat(label).valid).toBe(true);

    label.fields.push({
      slot: 0,
      field_type: 1,
      orientation: 0,
      style: 49,
      x1: 20,
      y1: 20,
      x2: 500,
      y2: 90,
      font: "C",
      dsr: 0,
    });
    expect(findNextFreeSlot(label.fields)).toBe(1);
    expect(validateLabelFormat(label).valid).toBe(true);

    // Invalid PFID, width, duplicate slot, inverted coords
    const bad = {
      ...label,
      pfid: 120,
      orientation: 5,
      width_hundredths_mm: 500,
      name: "A".repeat(30),
      fields: [
        ...label.fields,
        {
          slot: 0,
          field_type: 10,
          orientation: 0,
          style: 49,
          x1: 300,
          y1: 100,
          x2: 100,
          y2: 50,
          font: "2",
          dsr: 1,
        },
      ],
    };
    const res = validateLabelFormat(bad);
    expect(res.valid).toBe(false);
    expect(res.errors.length).toBeGreaterThanOrEqual(5);
  });

  it("resolves live preview text from a real PLU record", () => {
    const mk = (ft: number, dsr = 0) => ({
      slot: 0,
      field_type: ft,
      orientation: 0,
      style: 49,
      x1: 0,
      y1: 0,
      x2: 100,
      y2: 50,
      font: "B",
      dsr,
    });

    expect(resolveFieldPreviewText(mk(1), SAMPLE_PLU, 0.5)).toBe("ALITA DE POLLO ADOBADA");
    expect(resolveFieldPreviewText(mk(2), SAMPLE_PLU, 0.5)).toBe("Pollo marinado");
    expect(resolveFieldPreviewText(mk(3), SAMPLE_PLU, 0.5)).toBe("Mantener a 4C");
    expect(resolveFieldPreviewText(mk(5), SAMPLE_PLU, 0.5)).toMatch(/^EMP \d{4}-\d{2}-\d{2}$/);
    expect(resolveFieldPreviewText(mk(6), SAMPLE_PLU, 0.5)).toMatch(/^CAD \d{4}-\d{2}-\d{2}$/);
    expect(resolveFieldPreviewText(mk(7), SAMPLE_PLU, 0.5)).toBe("$70.00/kg");
    expect(resolveFieldPreviewText(mk(8), SAMPLE_PLU, 0.5)).toBe("0.500 kg");
    expect(resolveFieldPreviewText(mk(9), SAMPLE_PLU, 0.5)).toBe("$35.00");
    expect(resolveFieldPreviewText(mk(10), SAMPLE_PLU, 0.5)).toBe("2101001000000");
    expect(resolveFieldPreviewText(mk(11, 3), SAMPLE_PLU, 0.5)).toBe("MSG #3");
    expect(resolveFieldPreviewText(mk(15), SAMPLE_PLU, 0.5)).toBe("T 20g");
    expect(resolveFieldPreviewText(mk(53, 1), SAMPLE_PLU, 0.5)).toBe("LOGO #1");
    expect(resolveFieldPreviewText(mk(57), SAMPLE_PLU, 0.5)).toBe("");
    expect(resolveFieldPreviewText(mk(58), SAMPLE_PLU, 0.5)).toBe("");
    expect(resolveFieldPreviewText(mk(99), null, 0.5)).toBe("FT 99");
  });
});
