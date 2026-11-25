import type { BarcodeFormat, Plu } from "./types.js";

export interface XsBarcodeFormatDef {
  bmc: number; // 0=Label, 1=Receipt, 2=Stub, 3=Promo Voucher
  bt: number; // 0=EAN-8, 1=UPC-12, 2=UPC-13, 3=EAN-13, 5=EAN-128, 6=Code39, 7=Code128, 8=ITF-14
  bpd: number; // Price divisor: 1, 10, 100
  subtype: number; // Usually 3 (CVN 5000)
  template: string; // e.g. "2NNAAAAWWWWWC" or "02AAAAVPPPPPC"
}

export const BARCODE_CATEGORIES = [
  { value: 0, label: "Etiqueta (Label)" },
  { value: 1, label: "Recibo (Receipt)" },
  { value: 2, label: "Talón (Stub)" },
  { value: 3, label: "Vale Promocional (Voucher)" },
] as const;

export const BARCODE_SYMBOLOGIES = [
  { value: 0, label: "EAN-8", expectedLen: 8 },
  { value: 1, label: "UPC-12 (UPC-A)", expectedLen: 12 },
  { value: 2, label: "UPC-13", expectedLen: 13 },
  { value: 3, label: "EAN-13", expectedLen: 13 },
  { value: 5, label: "EAN-128 / GS1-128", expectedLen: null },
  { value: 6, label: "Code 39", expectedLen: null },
  { value: 7, label: "Code 128", expectedLen: null },
  { value: 8, label: "ITF-14", expectedLen: 14 },
  { value: 12, label: "GS1 DataBar Expanded", expectedLen: null },
  { value: 13, label: "GS1 DataBar-14", expectedLen: 14 },
] as const;

export const BARCODE_TEMPLATE_TOKENS = [
  { token: "N", label: "Depto (DID)", color: "#0284c7" },
  { token: "A", label: "Artículo / PLU", color: "#2563eb" },
  { token: "W", label: "Peso (g)", color: "#059669" },
  { token: "P", label: "Precio Total", color: "#d97706" },
  { token: "U", label: "Precio Unit.", color: "#7c3aed" },
  { token: "Q", label: "Cantidad / Pz", color: "#0d9488" },
  { token: "V", label: "Verif. interno", color: "#db2777" },
  { token: "C", label: "Checksum EAN", color: "#dc2626" },
] as const;

export interface ResolvedBarcodeSegment {
  token: string;
  value: string;
  color: string;
}

export interface ResolvedBarcode {
  code: string;
  segments: ResolvedBarcodeSegment[];
  symbology: string;
  valid: boolean;
  warnings: string[];
}

export function parseBarcodeDefinition(fmt: BarcodeFormat | null | undefined): XsBarcodeFormatDef {
  if (!fmt?.definition_json) {
    return { bmc: 0, bt: 3, bpd: 1, subtype: 3, template: "2NNAAAAWWWWWC" };
  }
  try {
    const parsed = JSON.parse(fmt.definition_json) as Partial<XsBarcodeFormatDef>;
    return {
      bmc: typeof parsed.bmc === "number" ? parsed.bmc : 0,
      bt: typeof parsed.bt === "number" ? parsed.bt : 3,
      bpd: typeof parsed.bpd === "number" && parsed.bpd > 0 ? parsed.bpd : 1,
      subtype: typeof parsed.subtype === "number" ? parsed.subtype : 3,
      template: typeof parsed.template === "string" ? parsed.template : "2NNAAAAWWWWWC",
    };
  } catch {
    return { bmc: 0, bt: 3, bpd: 1, subtype: 3, template: "2NNAAAAWWWWWC" };
  }
}

/** Standard GS1 / EAN / UPC Mod-10 check digit (weights 3,1 from right to left). */
export function computeEanCheckDigit(digits: string): string {
  const clean = digits.replace(/\D/g, "");
  let sum = 0;
  for (let i = 0; i < clean.length; i++) {
    const d = Number(clean[clean.length - 1 - i]);
    sum += i % 2 === 0 ? d * 3 : d;
  }
  const mod = sum % 10;
  return String(mod === 0 ? 0 : 10 - mod);
}

/** Internal price/weight check digit used by retail scales (4-digit or 5-digit weighted Mod-10). */
export function computeInternalCheckDigit(valueDigits: string): string {
  const weights4 = [2, 2, 3, 5];
  const weights5 = [5, 2, 5, 2, 5];
  const weights = valueDigits.length === 4 ? weights4 : weights5;
  let sum = 0;
  for (let i = 0; i < valueDigits.length; i++) {
    const d = Number(valueDigits[i]) || 0;
    const w = weights[i % weights.length] ?? 3;
    const prod = d * w;
    sum += prod >= 10 ? Math.floor(prod / 10) + (prod % 10) : prod;
  }
  return String((10 - (sum % 10)) % 10);
}

function padRightSlice(num: number, len: number): string {
  const s = String(Math.max(0, Math.round(num)));
  return s.length >= len ? s.slice(s.length - len) : s.padStart(len, "0");
}

function tokenColor(ch: string): string {
  if (ch >= "0" && ch <= "9") {
    return "#475569";
  }
  return BARCODE_TEMPLATE_TOKENS.find((t) => t.token === ch)?.color ?? "#64748b";
}

/**
 * Resolves an Avery Berkel `BFD` barcode template using a real `Plu` and sample weight/qty.
 */
export function resolveBarcodeTemplate(
  def: XsBarcodeFormatDef,
  plu: Plu | null,
  sampleWeightKg = 0.5,
): ResolvedBarcode {
  const template = (def.template || "").trim().toUpperCase();
  const warnings: string[] = [];
  const sym = BARCODE_SYMBOLOGIES.find((s) => s.value === def.bt);
  const symbology = sym?.label ?? `BT ${def.bt}`;

  if (sym?.expectedLen && template.length !== sym.expectedLen) {
    warnings.push(
      `La plantilla tiene ${template.length} caracteres; ${symbology} requiere ${sym.expectedLen}.`,
    );
  }

  const did = plu?.key.did ?? 1;
  const pluNum = plu?.key.plu ?? 1001;
  const unitPriceCents = plu?.price_1 ?? 7000;
  const weightGrams = Math.round(sampleWeightKg * 1000);
  const divisor = def.bpd > 0 ? def.bpd : 1;
  const totalPriceScaled = Math.round((unitPriceCents * sampleWeightKg) / divisor);
  const unitPriceScaled = Math.round(unitPriceCents / divisor);

  // Group consecutive identical tokens
  const runs: Array<{ token: string; len: number }> = [];
  for (const ch of template) {
    const last = runs[runs.length - 1];
    if (last && last.token === ch) {
      last.len += 1;
    } else {
      runs.push({ token: ch, len: 1 });
    }
  }

  // First pass: resolve all non-check tokens
  const segments: ResolvedBarcodeSegment[] = runs.map((run) => {
    let value: string;
    switch (run.token) {
      case "N":
        value = padRightSlice(did, run.len);
        break;
      case "A":
        value = padRightSlice(pluNum, run.len);
        break;
      case "W":
        value = padRightSlice(weightGrams, run.len);
        break;
      case "P":
        value = padRightSlice(totalPriceScaled, run.len);
        break;
      case "U":
        value = padRightSlice(unitPriceScaled, run.len);
        break;
      case "Q":
        value = padRightSlice(Math.max(1, Math.round(sampleWeightKg)), run.len);
        break;
      case "V":
      case "C":
        value = "0".repeat(run.len);
        break;
      default:
        value = run.token.repeat(run.len);
        break;
    }
    return { token: run.token, value, color: tokenColor(run.token) };
  });

  // Second pass: compute internal check digit 'V' from following P/W segment if present
  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i];
    if (seg?.token === "V") {
      const targetSeg = segments.slice(i + 1).find((s) => s.token === "P" || s.token === "W");
      const digit = computeInternalCheckDigit(targetSeg?.value ?? "0000");
      segments[i] = { ...seg, value: digit.repeat(seg.value.length) };
    }
  }

  // Third pass: compute final EAN/UPC Mod-10 check digit 'C'
  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i];
    if (seg?.token === "C") {
      const prefix = segments
        .slice(0, i)
        .map((s) => s.value)
        .join("");
      const digit = computeEanCheckDigit(prefix);
      segments[i] = { ...seg, value: digit.repeat(seg.value.length) };
    }
  }

  const code = segments.map((s) => s.value).join("");
  return {
    code,
    segments,
    symbology,
    valid: warnings.length === 0 && code.length > 0,
    warnings,
  };
}

// --- Pure SVG 1D Barcode Encoder (EAN-13, EAN-8, UPC-A, Code 128B fallback) ---

const EAN_L = [
  "0001101",
  "0011001",
  "0010011",
  "0111101",
  "0100011",
  "0110001",
  "0101111",
  "0111011",
  "0110111",
  "0001011",
];
const EAN_G = [
  "0100111",
  "0110011",
  "0011011",
  "0100001",
  "0011101",
  "0111001",
  "0000101",
  "0010001",
  "0001001",
  "0010111",
];
const EAN_R = [
  "1110010",
  "1100110",
  "1101100",
  "1000010",
  "1011100",
  "1001110",
  "1010000",
  "1000100",
  "1001000",
  "1110100",
];
const EAN13_PARITY = [
  "LLLLLL",
  "LLGLGG",
  "LLGGLG",
  "LLGGGL",
  "LGLLGG",
  "LGGLLG",
  "LGGGLL",
  "LGLGLG",
  "LGLGGL",
  "LGGLGL",
];

function encodeEan13Bits(code: string): string | null {
  const digits = code.replace(/\D/g, "");
  if (digits.length !== 13) {
    return null;
  }
  const first = Number(digits[0]);
  const parity = EAN13_PARITY[first] ?? "LLLLLL";
  let bits = "101";
  for (let i = 1; i <= 6; i++) {
    const d = Number(digits[i]);
    bits += parity[i - 1] === "L" ? EAN_L[d] : EAN_G[d];
  }
  bits += "01010";
  for (let i = 7; i <= 12; i++) {
    const d = Number(digits[i]);
    bits += EAN_R[d];
  }
  bits += "101";
  return bits;
}

function encodeEan8Bits(code: string): string | null {
  const digits = code.replace(/\D/g, "");
  if (digits.length !== 8) {
    return null;
  }
  let bits = "101";
  for (let i = 0; i < 4; i++) {
    bits += EAN_L[Number(digits[i])];
  }
  bits += "01010";
  for (let i = 4; i < 8; i++) {
    bits += EAN_R[Number(digits[i])];
  }
  bits += "101";
  return bits;
}

function encodeUpcABits(code: string): string | null {
  const digits = code.replace(/\D/g, "");
  if (digits.length !== 12) {
    return null;
  }
  return encodeEan13Bits(`0${digits}`);
}

/** Simple deterministic 1D bar pattern fallback for Code 128 / Code 39 / ITF-14 preview. */
function encodeGeneric1DBits(code: string): string {
  let bits = "11010010000"; // Code 128B Start
  for (let i = 0; i < code.length; i++) {
    const v = code.charCodeAt(i);
    // Deterministic 11-module bar pattern per character
    bits += "1";
    bits += v & 1 ? "10" : "01";
    bits += v & 2 ? "10" : "00";
    bits += v & 4 ? "11" : "10";
    bits += v & 8 ? "01" : "10";
    bits += "00";
  }
  bits += "1100011101011"; // Stop
  return bits;
}

export interface BarcodeBarRect {
  x: number;
  width: number;
}

export interface BarcodeSvgLayout {
  totalModules: number;
  bars: BarcodeBarRect[];
}

/**
 * Generates exact bar positions (in module units) for rendering an authentic SVG barcode.
 */
export function encodeBarcodeToBars(code: string, bt: number): BarcodeSvgLayout {
  let bits: string | null = null;
  if (bt === 3 || bt === 2) {
    bits = encodeEan13Bits(code);
  } else if (bt === 0) {
    bits = encodeEan8Bits(code);
  } else if (bt === 1) {
    bits = encodeUpcABits(code);
  }
  if (!bits) {
    bits = encodeGeneric1DBits(code || "0000000000000");
  }

  const bars: BarcodeBarRect[] = [];
  let i = 0;
  while (i < bits.length) {
    if (bits[i] === "1") {
      const start = i;
      while (i < bits.length && bits[i] === "1") {
        i++;
      }
      bars.push({ x: start, width: i - start });
    } else {
      i++;
    }
  }
  return { totalModules: bits.length, bars };
}
