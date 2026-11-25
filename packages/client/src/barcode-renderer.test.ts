import { describe, expect, it } from "vitest";
import {
  computeEanCheckDigit,
  computeInternalCheckDigit,
  encodeBarcodeToBars,
  parseBarcodeDefinition,
  resolveBarcodeTemplate,
} from "./barcode-renderer.js";
import type { BarcodeFormat, Plu } from "./types.js";

function samplePlu(): Plu {
  return {
    id: "plu-1",
    key: { did: 3, plu: 42 },
    description: "Jamón Serrano",
    button_text: null,
    price_1: 12500,
    price_2: 0,
    price_3: 0,
    product_type: "by_weight",
    ean: null,
    sku: null,
    group_id: 1,
    barcode_format_id: 1,
    label_format_id: 1,
    tare_id: null,
    tare_grams: null,
    date_offset_1: 7,
    date_print_control_1: 0,
    date_offset_2: 5,
    date_print_control_2: 0,
    tax_rate_id_1: 0,
    tax_rate_id_2: 0,
    text_areas: [],
    delta_hash: "hash-1",
    meta: {
      store_id: "store-1",
      sync_version: 1,
      updated_at: "2026-01-01T00:00:00Z",
      deleted_at: null,
    },
  };
}

describe("barcode-renderer", () => {
  it("computes standard EAN-13 check digit correctly including mod 0 case", () => {
    expect(computeEanCheckDigit("201000100500")).toBe("1");
    expect(computeEanCheckDigit("000000000000")).toBe("0");
  });

  it("computes 4-digit and 5-digit internal check digits", () => {
    expect(computeInternalCheckDigit("9876")).toMatch(/^\d$/);
    expect(computeInternalCheckDigit("12345")).toMatch(/^\d$/);
  });

  it("parses barcode definitions with defaults, partial overrides, and malformed JSON", () => {
    expect(parseBarcodeDefinition(null)).toEqual({
      bmc: 0,
      bt: 3,
      bpd: 1,
      subtype: 3,
      template: "2NNAAAAWWWWWC",
    });

    const validFmt: BarcodeFormat = {
      id: "bf-1",
      bid: 1,
      name: "Custom",
      definition_json: JSON.stringify({
        bmc: 1,
        bt: 1,
        bpd: 10,
        subtype: 2,
        template: "02AAAAVPPPPPC",
      }),
      meta: {
        store_id: "store-1",
        sync_version: 1,
        updated_at: "2026-01-01T00:00:00Z",
        deleted_at: null,
      },
    };
    expect(parseBarcodeDefinition(validFmt)).toEqual({
      bmc: 1,
      bt: 1,
      bpd: 10,
      subtype: 2,
      template: "02AAAAVPPPPPC",
    });

    expect(
      parseBarcodeDefinition({
        ...validFmt,
        definition_json: JSON.stringify({ bpd: 0 }),
      }),
    ).toEqual({
      bmc: 0,
      bt: 3,
      bpd: 1,
      subtype: 3,
      template: "2NNAAAAWWWWWC",
    });

    expect(
      parseBarcodeDefinition({
        ...validFmt,
        definition_json: "{invalid json",
      }),
    ).toEqual({
      bmc: 0,
      bt: 3,
      bpd: 1,
      subtype: 3,
      template: "2NNAAAAWWWWWC",
    });
  });

  it("resolves BFD weight template and encodes 95-module EAN-13 SVG bars", () => {
    const resolved = resolveBarcodeTemplate(
      { bmc: 0, bt: 3, bpd: 1, subtype: 3, template: "2NNAAAAWWWWWC" },
      null,
      0.5,
    );
    expect(resolved.code).toHaveLength(13);
    expect(resolved.valid).toBe(true);
    const svg = encodeBarcodeToBars(resolved.code, 3);
    expect(svg.totalModules).toBe(95);
    expect(svg.bars.length).toBeGreaterThan(20);
  });

  it("resolves price, unit price, quantity, internal check digit, and literal tokens with a real PLU", () => {
    const plu = samplePlu();
    const resolved = resolveBarcodeTemplate(
      { bmc: 0, bt: 3, bpd: 10, subtype: 3, template: "02AAAAVPPPPPC" },
      plu,
      0.5,
    );
    expect(resolved.code).toHaveLength(13);
    expect(resolved.valid).toBe(true);
    expect(resolved.segments.some((s) => s.token === "V")).toBe(true);
    expect(resolved.segments.some((s) => s.token === "C")).toBe(true);

    const extra = resolveBarcodeTemplate(
      { bmc: 0, bt: 99, bpd: 0, subtype: 3, template: "UUUQQVX" },
      plu,
      2.4,
    );
    expect(extra.symbology).toBe("BT 99");
    expect(extra.valid).toBe(true);
    expect(extra.segments.some((s) => s.token === "U")).toBe(true);
    expect(extra.segments.some((s) => s.token === "Q")).toBe(true);
  });

  it("reports warnings when template length mismatches fixed-length symbology", () => {
    const resolved = resolveBarcodeTemplate(
      { bmc: 0, bt: 0, bpd: 1, subtype: 3, template: "2NNAAAA" },
      null,
      1,
    );
    expect(resolved.valid).toBe(false);
    expect(resolved.warnings).toHaveLength(1);
  });

  it("encodes EAN-8, UPC-A, and generic 1D fallbacks", () => {
    const ean8 = encodeBarcodeToBars("12345670", 0);
    expect(ean8.totalModules).toBe(67);
    expect(ean8.bars.length).toBeGreaterThan(10);

    const upcA = encodeBarcodeToBars("012345678905", 1);
    expect(upcA.totalModules).toBe(95);

    // Invalid lengths fall back to generic 1D encoder
    const badEan13 = encodeBarcodeToBars("123", 3);
    expect(badEan13.totalModules).toBeGreaterThan(30);

    const badEan8 = encodeBarcodeToBars("123", 0);
    expect(badEan8.totalModules).toBeGreaterThan(30);

    const badUpcA = encodeBarcodeToBars("123", 1);
    expect(badUpcA.totalModules).toBeGreaterThan(30);

    const generic = encodeBarcodeToBars("CODE128", 7);
    expect(generic.totalModules).toBe(11 + 7 * 11 + 13);

    const emptyFallback = encodeBarcodeToBars("", 7);
    expect(emptyFallback.totalModules).toBe(11 + 13 * 11 + 13);
  });
});
