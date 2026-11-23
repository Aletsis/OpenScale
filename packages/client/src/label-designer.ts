import type { Plu } from "./types.js";

export const LABEL_FORMAT_BYTE_LIMIT = 898;
export const LABEL_MAX_SLOTS = 50;
export const LABEL_MAX_NAME_LEN = 27;

export interface LabelFieldModel {
  slot: number;
  field_type: number;
  orientation: number; // 0=0°, 1=90°, 2=180°, 3=270°
  style: number; // 49='1', 50='2', 51='3'
  x1: number; // tenths of mm
  y1: number; // tenths of mm
  x2: number; // tenths of mm
  y2: number; // tenths of mm
  font: string; // single ABML char ('0'..'8', 'A'..'X')
  dsr: number; // table reference (BID / Message ID / Logo ID)
}

export interface LabelFormatModel {
  pfid: number;
  orientation: number;
  active: boolean;
  width_hundredths_mm: number;
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
  { ft: 15, label: "Tare Weight", defaultWidthMm: 16, defaultHeightMm: 5, defaultFont: "1" },
  { ft: 53, label: "Logo / BMP", defaultWidthMm: 16, defaultHeightMm: 10, defaultFont: "0" },
  { ft: 57, label: "Line", defaultWidthMm: 52, defaultHeightMm: 1, defaultFont: "0" },
  { ft: 58, label: "Box", defaultWidthMm: 52, defaultHeightMm: 12, defaultFont: "0" },
] as const;

export interface AbmlFontSpec {
  code: string;
  label: string;
  pxSize: number;
  bold: boolean;
}

export const ABML_FONTS: readonly AbmlFontSpec[] = [
  { code: "1", label: "1 — Small (8pt)", pxSize: 10, bold: false },
  { code: "2", label: "2 — Medium (10pt)", pxSize: 12, bold: false },
  { code: "3", label: "3 — Large (12pt)", pxSize: 14, bold: false },
  { code: "4", label: "4 — XL (14pt)", pxSize: 16, bold: true },
  { code: "A", label: "A — Bold Small", pxSize: 11, bold: true },
  { code: "B", label: "B — Bold Medium", pxSize: 13, bold: true },
  { code: "C", label: "C — Bold Title", pxSize: 16, bold: true },
  { code: "D", label: "D — Display Large", pxSize: 18, bold: true },
  { code: "E", label: "E — Price Hero", pxSize: 22, bold: true },
] as const;

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
      pxSize: 12,
      bold: false,
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
    case 10:
      return plu?.ean || "2101001000000";
    case 11:
      return `MSG #${field.dsr || 1}`;
    case 15:
      return `T ${(plu?.tare_grams ?? 15).toString()}g`;
    case 53:
      return `LOGO #${field.dsr}`;
    case 57:
      return "";
    case 58:
      return "";
    default:
      return `FT ${field.field_type}`;
  }
}
