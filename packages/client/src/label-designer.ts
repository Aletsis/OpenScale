import { parseBarcodeDefinition, resolveBarcodeTemplate } from "./barcode-renderer.js";
import type { BarcodeFormat, LabelFormat, Plu, StoredText } from "./types.js";

export const LABEL_FORMAT_BYTE_LIMIT = 898;
export const LABEL_MAX_SLOTS = 50;
export const LABEL_MAX_NAME_LEN = 27;

export interface LabelFieldModel {
  slot: number;
  field_type: number;
  orientation: number; // 0=0°, 1=90°, 2=180°, 3=270°
  style: number; // 49='1' (Left), 50='2' (Center), 51='3' (Right)
  x1: number; // tenths of mm
  y1: number; // tenths of mm
  x2: number; // tenths of mm
  y2: number; // tenths of mm
  font: string; // single ABML char ('0'..'8', 'A'..'X')
  dsr: number; // table reference (BID / Message ID / Logo ID)
  extra?: number[]; // 4 trailing bytes (+13..+16) preserved for lossless roundtrip
}

export interface LabelFormatModel {
  pfid: number;
  orientation: number;
  active: boolean;
  width_hundredths_mm: number;
  height_tenths_mm?: number;
  name: string;
  fields: LabelFieldModel[];
  raw: number[];
}

export interface FieldTypeMeta {
  ft: number;
  label: string;
  defaultWidthMm: number;
  defaultHeightMm: number;
  defaultFont: string;
}

export const LABEL_FIELD_TYPES: readonly FieldTypeMeta[] = [
  {
    ft: 1,
    label: "Description (Text 1)",
    defaultWidthMm: 50,
    defaultHeightMm: 7,
    defaultFont: "C",
  },
  {
    ft: 2,
    label: "Ingredients (Text 2)",
    defaultWidthMm: 50,
    defaultHeightMm: 6,
    defaultFont: "2",
  },
  { ft: 3, label: "Extra Text (Text 3)", defaultWidthMm: 50, defaultHeightMm: 5, defaultFont: "1" },
  { ft: 5, label: "Date 1 (Pack/Sell)", defaultWidthMm: 22, defaultHeightMm: 5, defaultFont: "2" },
  { ft: 6, label: "Date 2 (Expiry)", defaultWidthMm: 22, defaultHeightMm: 5, defaultFont: "2" },
  { ft: 7, label: "Unit Price 1", defaultWidthMm: 18, defaultHeightMm: 6, defaultFont: "B" },
  { ft: 8, label: "Weight / Qty", defaultWidthMm: 18, defaultHeightMm: 6, defaultFont: "B" },
  { ft: 9, label: "Total Price 1", defaultWidthMm: 22, defaultHeightMm: 8, defaultFont: "E" },
  { ft: 10, label: "Barcode (EAN)", defaultWidthMm: 44, defaultHeightMm: 12, defaultFont: "2" },
  { ft: 11, label: "Stored Text 1", defaultWidthMm: 48, defaultHeightMm: 5, defaultFont: "2" },
  { ft: 12, label: "Stored Text 2", defaultWidthMm: 48, defaultHeightMm: 5, defaultFont: "2" },
  { ft: 13, label: "Stored Text 3", defaultWidthMm: 48, defaultHeightMm: 5, defaultFont: "1" },
  { ft: 14, label: "PLU Number", defaultWidthMm: 16, defaultHeightMm: 5, defaultFont: "2" },
  { ft: 15, label: "Tare Weight", defaultWidthMm: 16, defaultHeightMm: 5, defaultFont: "1" },
  { ft: 21, label: "Total Price 2", defaultWidthMm: 22, defaultHeightMm: 7, defaultFont: "D" },
  { ft: 53, label: "Logo / BMP", defaultWidthMm: 16, defaultHeightMm: 10, defaultFont: "0" },
  { ft: 57, label: "Line", defaultWidthMm: 52, defaultHeightMm: 1, defaultFont: "0" },
  { ft: 58, label: "Box", defaultWidthMm: 52, defaultHeightMm: 12, defaultFont: "0" },
  { ft: 142, label: "Nutrition Table", defaultWidthMm: 50, defaultHeightMm: 18, defaultFont: "1" },
] as const;

export interface AbmlFontSpec {
  code: string;
  label: string;
  /** Physical glyph height on thermal paper in millimeters (8 dots/mm = 203 dpi). */
  heightMm: number;
  pxSize: number;
  bold: boolean;
}

export const ABML_FONTS: readonly AbmlFontSpec[] = [
  { code: "0", label: "0 — Graphic / None", heightMm: 2.0, pxSize: 10, bold: false },
  { code: "1", label: "1 — Micro (1.8 mm)", heightMm: 1.8, pxSize: 9, bold: false },
  { code: "2", label: "2 — Small (2.4 mm)", heightMm: 2.4, pxSize: 12, bold: false },
  { code: "3", label: "3 — Medium (3.0 mm)", heightMm: 3.0, pxSize: 15, bold: false },
  { code: "4", label: "4 — Large (3.8 mm)", heightMm: 3.8, pxSize: 19, bold: true },
  { code: "5", label: "5 — XL (4.5 mm)", heightMm: 4.5, pxSize: 22, bold: true },
  { code: "6", label: "6 — Display (5.2 mm)", heightMm: 5.2, pxSize: 26, bold: true },
  { code: "A", label: "A — Bold Small (2.2 mm)", heightMm: 2.2, pxSize: 11, bold: true },
  { code: "B", label: "B — Bold Medium (2.8 mm)", heightMm: 2.8, pxSize: 14, bold: true },
  { code: "C", label: "C — Bold Title (3.4 mm)", heightMm: 3.4, pxSize: 17, bold: true },
  { code: "D", label: "D — Display Large (4.2 mm)", heightMm: 4.2, pxSize: 21, bold: true },
  { code: "E", label: "E — Price Hero (5.0 mm)", heightMm: 5.0, pxSize: 22, bold: true },
  { code: "F", label: "F — Extra Bold (5.8 mm)", heightMm: 5.8, pxSize: 29, bold: true },
] as const;

export function fieldStyleAlignment(style: number): "left" | "center" | "right" {
  if (style === 50 || style === 2) {
    return "center";
  }
  if (style === 51 || style === 3) {
    return "right";
  }
  return "left";
}

export function mmToTenths(mm: number): number {
  return Math.max(0, Math.round(mm * 10));
}

export function tenthsToMm(tenths: number): number {
  return Math.round(tenths) / 10;
}

export function mmToHundredths(mm: number): number {
  return Math.max(0, Math.round(mm * 100));
}

export function hundredthsToMm(hundredths: number): number {
  return Math.round(hundredths) / 100;
}

export function rotationDegrees(orientation: number): number {
  const normalized = ((orientation % 4) + 4) % 4;
  return normalized * 90;
}

export function getAbmlFontSpec(code: string): AbmlFontSpec {
  return (
    ABML_FONTS.find((f) => f.code === code) ?? {
      code,
      label: `${code} — Custom`,
      heightMm: 2.6,
      pxSize: 12,
      bold: code >= "A" && code <= "Z",
    }
  );
}

export function findNextFreeSlot(fields: readonly LabelFieldModel[]): number | null {
  const used = new Set(fields.map((f) => f.slot));
  for (let slot = 0; slot < LABEL_MAX_SLOTS; slot++) {
    if (!used.has(slot)) {
      return slot;
    }
  }
  return null;
}

export function createBlankLabelFormat(pfid = 98, widthMm = 59): LabelFormatModel {
  return {
    pfid,
    orientation: 0,
    active: true,
    width_hundredths_mm: mmToHundredths(widthMm),
    name: `Custom ${widthMm}mm`,
    fields: [],
    raw: new Array<number>(LABEL_FORMAT_BYTE_LIMIT).fill(0),
  };
}

export function estimateLabelHeightMm(
  fields: readonly LabelFieldModel[],
  minHeightMm = 40,
): number {
  const maxY = fields.reduce((acc, f) => Math.max(acc, tenthsToMm(f.y2)), 0);
  return Math.max(minHeightMm, Math.ceil(maxY + 4));
}

export function findNextFreePfid(existingPfids: readonly number[], preferred = 98): number {
  const used = new Set(existingPfids);
  if (preferred >= 0 && preferred <= 99 && !used.has(preferred)) {
    return preferred;
  }
  for (let pfid = 1; pfid <= 99; pfid++) {
    if (!used.has(pfid)) {
      return pfid;
    }
  }
  return used.has(0) ? preferred : 0;
}

/**
 * Converts a domain `LabelFormat` (from SQLite or a scale actor) into an editable `LabelFormatModel`.
 */
export function labelFormatToModel(fmt: LabelFormat): LabelFormatModel {
  let orientation = 0;
  let active = true;
  let widthHundredths = fmt.width_tenths_mm > 0 ? fmt.width_tenths_mm * 10 : 5900;
  let fields: LabelFieldModel[] = [];

  try {
    const parsed = JSON.parse(fmt.definition_json) as {
      orientation?: unknown;
      active?: unknown;
      width_hundredths_mm?: unknown;
      fields?: unknown;
    };
    if (typeof parsed.orientation === "number") {
      orientation = parsed.orientation;
    }
    if (typeof parsed.active === "boolean") {
      active = parsed.active;
    }
    if (typeof parsed.width_hundredths_mm === "number" && parsed.width_hundredths_mm > 0) {
      widthHundredths = parsed.width_hundredths_mm;
    }
    if (Array.isArray(parsed.fields)) {
      fields = parsed.fields as LabelFieldModel[];
    }
  } catch {
    // Fallback to defaults if definition_json is empty or malformed
  }

  const raw =
    fmt.raw_binary && fmt.raw_binary.length >= LABEL_FORMAT_BYTE_LIMIT
      ? fmt.raw_binary.slice(0, LABEL_FORMAT_BYTE_LIMIT)
      : new Array<number>(LABEL_FORMAT_BYTE_LIMIT).fill(0);

  return {
    pfid: fmt.pfid,
    orientation,
    active,
    width_hundredths_mm: widthHundredths,
    height_tenths_mm: fmt.height_tenths_mm > 0 ? fmt.height_tenths_mm : undefined,
    name: fmt.name,
    fields,
    raw,
  };
}

/**
 * Converts an edited `LabelFormatModel` back into a domain `LabelFormat` for scale write & SQLite persistence.
 */
export function modelToLabelFormat(
  model: LabelFormatModel,
  storeId: string,
  existing?: LabelFormat | null,
): LabelFormat {
  const now = new Date().toISOString();
  const definition_json = JSON.stringify({
    orientation: model.orientation,
    active: model.active,
    width_hundredths_mm: model.width_hundredths_mm,
    fields: model.fields,
  });

  const raw_binary =
    model.raw.length >= LABEL_FORMAT_BYTE_LIMIT
      ? model.raw.slice(0, LABEL_FORMAT_BYTE_LIMIT)
      : new Array<number>(LABEL_FORMAT_BYTE_LIMIT).fill(0);

  return {
    id: existing?.id ?? crypto.randomUUID(),
    pfid: model.pfid,
    name: model.name,
    width_tenths_mm: Math.round(model.width_hundredths_mm / 10),
    height_tenths_mm:
      model.height_tenths_mm && model.height_tenths_mm > 0
        ? model.height_tenths_mm
        : (existing?.height_tenths_mm ?? mmToTenths(estimateLabelHeightMm(model.fields))),
    definition_json,
    raw_binary,
    meta: existing
      ? {
          ...existing.meta,
          sync_version: existing.meta.sync_version + 1,
          updated_at: now,
        }
      : {
          store_id: storeId,
          sync_version: 1,
          updated_at: now,
          deleted_at: null,
        },
  };
}

export interface LabelValidationResult {
  valid: boolean;
  errors: string[];
  byteSize: number;
}

/**
 * Validates that a `LabelFormatModel` conforms to XS `LABEL_FORMATS` (`CMD 29`, 898 B) limits.
 */
export function validateLabelFormat(label: LabelFormatModel): LabelValidationResult {
  const errors: string[] = [];

  if (label.pfid < 0 || label.pfid > 99) {
    errors.push("PFID must be between 0 and 99.");
  }
  if (label.orientation < 0 || label.orientation > 3) {
    errors.push("Label orientation must be 0..3 (0°/90°/180°/270°).");
  }
  if (label.width_hundredths_mm < 1000 || label.width_hundredths_mm > 10000) {
    errors.push("Label width must be between 10.0 mm and 100.0 mm.");
  }
  if (label.name.length > LABEL_MAX_NAME_LEN) {
    errors.push(`Label name exceeds ${LABEL_MAX_NAME_LEN} characters.`);
  }
  if (label.fields.length > LABEL_MAX_SLOTS) {
    errors.push(`Cannot exceed ${LABEL_MAX_SLOTS} field slots.`);
  }

  const seenSlots = new Set<number>();
  for (const field of label.fields) {
    if (field.slot < 0 || field.slot >= LABEL_MAX_SLOTS) {
      errors.push(`Field slot ${field.slot} out of range (0..49).`);
    }
    if (seenSlots.has(field.slot)) {
      errors.push(`Duplicate field slot ${field.slot}.`);
    }
    seenSlots.add(field.slot);

    if (field.x2 < field.x1 || field.y2 < field.y1) {
      errors.push(`Field slot ${field.slot} has inverted coordinates (X2 < X1 or Y2 < Y1).`);
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    byteSize: LABEL_FORMAT_BYTE_LIMIT,
  };
}

/**
 * Resolves live preview text for a label field using a real `Plu` record and sample weight.
 */
export function resolveFieldPreviewText(
  field: LabelFieldModel,
  plu: Plu | null,
  sampleWeightKg = 0.5,
  storedTexts?: readonly StoredText[],
  barcodeFormats?: readonly BarcodeFormat[],
): string {
  const unitPrice = (plu?.price_1 ?? 7000) / 100;
  const totalPrice = unitPrice * sampleWeightKg;

  switch (field.field_type) {
    case 1:
      return plu?.description || "ALITA DE POLLO ADOBADA";
    case 2:
      return plu?.text_areas[0] || "Ingredientes: pollo fresco, especias";
    case 3:
      return plu?.text_areas[1] || "Conservar en refrigeración";
    case 5: {
      const d = new Date();
      return `EMP ${d.toISOString().slice(0, 10)}`;
    }
    case 6: {
      const d = new Date(Date.now() + (plu?.date_offset_1 ?? 7) * 86_400_000);
      return `CAD ${d.toISOString().slice(0, 10)}`;
    }
    case 7:
      return `$${unitPrice.toFixed(2)}/kg`;
    case 8:
      return `${sampleWeightKg.toFixed(3)} kg`;
    case 9:
      return `$${totalPrice.toFixed(2)}`;
    case 10: {
      const bid = field.dsr || plu?.barcode_format_id || 1;
      const fmt = barcodeFormats?.find((b) => b.bid === bid);
      if (fmt) {
        const def = parseBarcodeDefinition(fmt);
        return resolveBarcodeTemplate(def, plu, sampleWeightKg).code;
      }
      return plu?.ean || "2101001000000";
    }
    case 11:
    case 12:
    case 13: {
      const textId = field.dsr || 1;
      const found = storedTexts?.find((t) => t.text_id === textId);
      return found?.content || `MSG #${textId}`;
    }
    case 14:
      return `PLU ${plu?.key.plu ?? 1001}`;
    case 15:
      return `T ${(plu?.tare_grams ?? 15).toString()}g`;
    case 21:
      return `$${(((plu?.price_2 ?? 0) / 100) * sampleWeightKg).toFixed(2)}`;
    case 53:
      return `LOGO #${field.dsr}`;
    case 57:
      return "";
    case 58:
      return "";
    case 142:
      return "Información Nutrimental";
    default:
      return `FT ${field.field_type}`;
  }
}
