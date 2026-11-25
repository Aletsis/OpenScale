export interface ScaleSettings748 {
  version: string;
  bank1: string;
  bank2: string;
  bank3: string;
  bank4: string;
}

export type BankNumber = 1 | 2 | 3 | 4;
export type BankCategory = "metrology" | "printing" | "region" | "operation" | "system";
export type BankRiskLevel = "normal" | "caution" | "calibration";

export interface BankOption {
  value: string;
  label: string;
}

export interface BankFieldSpec {
  id: string;
  code: string;
  label: string;
  description: string;
  bank: BankNumber;
  /** 1-based start digit within the 16-digit bank string (1..16). */
  startDigit: number;
  /** Number of digits occupied by this parameter (1 or 2). */
  length: 1 | 2;
  category: BankCategory;
  risk: BankRiskLevel;
  verified: boolean;
  options?: readonly BankOption[];
}

export const BANK_CATEGORIES: ReadonlyArray<{ id: BankCategory; label: string }> = [
  { id: "metrology", label: "Balanza y Capacidad" },
  { id: "printing", label: "Impresión y Etiquetas" },
  { id: "region", label: "Moneda, Idioma y Región" },
  { id: "operation", label: "Operación y Teclado" },
  { id: "system", label: "Sistema y Periféricos" },
];

/**
 * Structured map of Avery Berkel XS / M-Series `#748 Scale Settings` (4 banks × 16 digits).
 * Verified against live XS200 (`APP v4.1.7.0 / PROTOCOL 8000`, `0001,8610011020214130,...`).
 * Every position across all 4 banks is covered so the user can inspect and edit any digit/pair
 * safely while preserving untouched positions.
 */
export const BANK_748_FIELDS: readonly BankFieldSpec[] = [
  // --- BANK 1 (16 digits: e.g. 8610011020214130) ---
  {
    id: "b1_capf",
    code: "CAPF",
    label: "Capacidad y División",
    description: "Capacidad máxima y resolución metrológica de la báscula (Banco 1, dígito 1).",
    bank: 1,
    startDigit: 1,
    length: 1,
    category: "metrology",
    risk: "calibration",
    verified: true,
    options: [
      { value: "4", label: "4 — 6 kg × 1 g" },
      { value: "5", label: "5 — 6 kg × 2 g" },
      { value: "6", label: "6 — 6 kg Doble Rango" },
      { value: "7", label: "7 — 15 kg × 2 g / 5 g" },
      { value: "8", label: "8 — 15 kg × 5 g (Estándar)" },
      { value: "9", label: "9 — 30 kg × 5 g / 10 g" },
    ],
  },
  {
    id: "b1_d2",
    code: "UNIT",
    label: "Unidad / Rango de Pesaje",
    description: "Configuración de unidad de peso y modo de rango (Banco 1, dígito 2).",
    bank: 1,
    startDigit: 2,
    length: 1,
    category: "metrology",
    risk: "calibration",
    verified: false,
    options: [
      { value: "0", label: "0 — Estándar" },
      { value: "6", label: "6 — Métrico (kg/g)" },
    ],
  },
  {
    id: "b1_dsm",
    code: "DSM",
    label: "Modo de Venta por Defecto (DSM)",
    description: "Comportamiento por defecto al seleccionar un PLU (Banco 1, dígito 3).",
    bank: 1,
    startDigit: 3,
    length: 1,
    category: "operation",
    risk: "normal",
    verified: true,
    options: [
      { value: "0", label: "0 — Libre / Manual" },
      { value: "1", label: "1 — PLU Pesado" },
      { value: "2", label: "2 — PLU Por Pieza" },
    ],
  },
  {
    id: "b1_d4_5",
    code: "B1_04",
    label: "Opciones de Tara / Cero (d4-5)",
    description: "Par de dígitos 4–5 del Banco 1 (comportamiento de tara y seguimiento de cero).",
    bank: 1,
    startDigit: 4,
    length: 2,
    category: "metrology",
    risk: "caution",
    verified: false,
  },
  {
    id: "b1_d6_7",
    code: "B1_06",
    label: "Opciones de Display y Redondeo (d6-7)",
    description: "Par de dígitos 6–7 del Banco 1 (visualización de peso/precio y redondeo).",
    bank: 1,
    startDigit: 6,
    length: 2,
    category: "operation",
    risk: "normal",
    verified: false,
  },
  {
    id: "b1_d8_9",
    code: "PM",
    label: "Modo de Impresora (d8-9 / PM)",
    description: "Modo de salida de impresora principal (ej. 02 = Etiqueta, Banco 1, dígitos 8–9).",
    bank: 1,
    startDigit: 8,
    length: 2,
    category: "printing",
    risk: "normal",
    verified: true,
    options: [
      { value: "00", label: "00 — Deshabilitada" },
      { value: "01", label: "01 — Recibo / Ticket" },
      { value: "02", label: "02 — Etiqueta (Label)" },
      { value: "03", label: "03 — Etiqueta y Recibo" },
    ],
  },
  {
    id: "b1_d10_11",
    code: "DPPF",
    label: "Decimales de Precio (d10-11 / DPPF)",
    description:
      "Posición del punto decimal en precios unitarios y totales (Banco 1, dígitos 10–11).",
    bank: 1,
    startDigit: 10,
    length: 2,
    category: "region",
    risk: "caution",
    verified: true,
    options: [
      { value: "00", label: "00 — 0 decimales ($100)" },
      { value: "01", label: "01 — 1 decimal ($10.0)" },
      { value: "02", label: "02 — 2 decimales ($10.00)" },
      { value: "03", label: "03 — 3 decimales ($10.000)" },
    ],
  },
  {
    id: "b1_d12_13",
    code: "B1_12",
    label: "Control de Impresión / Sensor (d12-13)",
    description: "Par de dígitos 12–13 del Banco 1 (detección de etiqueta y corte).",
    bank: 1,
    startDigit: 12,
    length: 2,
    category: "printing",
    risk: "normal",
    verified: false,
  },
  {
    id: "b1_d14_16",
    code: "B1_14",
    label: "Opciones Extendidas Banco 1 (d14-15)",
    description: "Par de dígitos 14–15 del Banco 1.",
    bank: 1,
    startDigit: 14,
    length: 2,
    category: "system",
    risk: "caution",
    verified: false,
  },
  {
    id: "b1_d16",
    code: "B1_16",
    label: "Flag Final Banco 1 (d16)",
    description: "Último dígito del Banco 1.",
    bank: 1,
    startDigit: 16,
    length: 1,
    category: "system",
    risk: "caution",
    verified: false,
  },

  // --- BANK 2 (16 digits: e.g. 0020020110111000) ---
  {
    id: "b2_d1_2",
    code: "B2_01",
    label: "Modo de Operador / Vendedores (d1-2)",
    description: "Par de dígitos 1–2 del Banco 2 (gestión de operadores y teclas de vendedor).",
    bank: 2,
    startDigit: 1,
    length: 2,
    category: "operation",
    risk: "normal",
    verified: false,
  },
  {
    id: "b2_d3_4",
    code: "B2_03",
    label: "Formato de Fecha / Pre-empaque (d3-4)",
    description: "Par de dígitos 3–4 del Banco 2.",
    bank: 2,
    startDigit: 3,
    length: 2,
    category: "printing",
    risk: "normal",
    verified: false,
  },
  {
    id: "b2_csy",
    code: "CSY",
    label: "Símbolo de Moneda (CSY)",
    description: "Símbolo de divisa impreso en etiquetas y tickets (Banco 2, dígitos 5–6).",
    bank: 2,
    startDigit: 5,
    length: 2,
    category: "region",
    risk: "normal",
    verified: true,
    options: [
      { value: "00", label: "00 — Sin símbolo" },
      { value: "01", label: "01 — £ (Libra)" },
      { value: "02", label: "02 — $ (Peso / Dólar)" },
      { value: "03", label: "03 — € (Euro)" },
    ],
  },
  {
    id: "b2_lnr",
    code: "LNR",
    label: "Idioma del Sistema (LNR)",
    description:
      "Idioma de menús, pantalla y textos internos de la báscula (Banco 2, dígitos 7–8).",
    bank: 2,
    startDigit: 7,
    length: 2,
    category: "region",
    risk: "normal",
    verified: true,
    options: [
      { value: "00", label: "00 — Inglés" },
      { value: "01", label: "01 — Español" },
      { value: "02", label: "02 — Francés" },
      { value: "03", label: "03 — Alemán" },
      { value: "04", label: "04 — Portugués" },
    ],
  },
  {
    id: "b2_d9_10",
    code: "B2_09",
    label: "Opciones de Código de Barras (d9-10)",
    description: "Par de dígitos 9–10 del Banco 2.",
    bank: 2,
    startDigit: 9,
    length: 2,
    category: "printing",
    risk: "normal",
    verified: false,
  },
  {
    id: "b2_d11_12",
    code: "B2_11",
    label: "Opciones de Recibo / Totales (d11-12)",
    description: "Par de dígitos 11–12 del Banco 2.",
    bank: 2,
    startDigit: 11,
    length: 2,
    category: "operation",
    risk: "normal",
    verified: false,
  },
  {
    id: "b2_d13_14",
    code: "B2_13",
    label: "Control de Precios / Descuentos (d13-14)",
    description: "Par de dígitos 13–14 del Banco 2.",
    bank: 2,
    startDigit: 13,
    length: 2,
    category: "operation",
    risk: "normal",
    verified: false,
  },
  {
    id: "b2_d15_16",
    code: "B2_15",
    label: "Reservado / Auxiliar Banco 2 (d15-16)",
    description: "Par de dígitos 15–16 del Banco 2.",
    bank: 2,
    startDigit: 15,
    length: 2,
    category: "system",
    risk: "caution",
    verified: false,
  },

  // --- BANK 3 (16 digits: e.g. 0110110100111000) ---
  {
    id: "b3_d1_2",
    code: "B3_01",
    label: "Configuración de Fecha 1 / Fecha 2 (d1-2)",
    description: "Par de dígitos 1–2 del Banco 3 (formato de fechas de empaque/caducidad).",
    bank: 3,
    startDigit: 1,
    length: 2,
    category: "printing",
    risk: "normal",
    verified: false,
  },
  {
    id: "b3_d3_4",
    code: "B3_03",
    label: "Opciones de Etiquetado / Trazabilidad (d3-4)",
    description: "Par de dígitos 3–4 del Banco 3.",
    bank: 3,
    startDigit: 3,
    length: 2,
    category: "printing",
    risk: "normal",
    verified: false,
  },
  {
    id: "b3_d5_6",
    code: "B3_05",
    label: "Comportamiento de Teclas / Acceso Directo (d5-6)",
    description: "Par de dígitos 5–6 del Banco 3.",
    bank: 3,
    startDigit: 5,
    length: 2,
    category: "operation",
    risk: "normal",
    verified: false,
  },
  {
    id: "b3_d7_8",
    code: "B3_07",
    label: "Cajón de Dinero / Escáner (d7-8)",
    description: "Par de dígitos 7–8 del Banco 3 (periféricos conectados).",
    bank: 3,
    startDigit: 7,
    length: 2,
    category: "system",
    risk: "normal",
    verified: false,
  },
  {
    id: "b3_d9_10",
    code: "B3_09",
    label: "Impuestos / IVA en Ticket (d9-10)",
    description: "Par de dígitos 9–10 del Banco 3.",
    bank: 3,
    startDigit: 9,
    length: 2,
    category: "region",
    risk: "normal",
    verified: false,
  },
  {
    id: "b3_d11_12",
    code: "B3_11",
    label: "Opciones de Seguridad / Bloqueos (d11-12)",
    description: "Par de dígitos 11–12 del Banco 3.",
    bank: 3,
    startDigit: 11,
    length: 2,
    category: "operation",
    risk: "caution",
    verified: false,
  },
  {
    id: "b3_d13_14",
    code: "B3_13",
    label: "Comunicación Inter-Báscula (d13-14)",
    description: "Par de dígitos 13–14 del Banco 3.",
    bank: 3,
    startDigit: 13,
    length: 2,
    category: "system",
    risk: "caution",
    verified: false,
  },
  {
    id: "b3_d15_16",
    code: "B3_15",
    label: "Reservado / Auxiliar Banco 3 (d15-16)",
    description: "Par de dígitos 15–16 del Banco 3.",
    bank: 3,
    startDigit: 15,
    length: 2,
    category: "system",
    risk: "caution",
    verified: false,
  },

  // --- BANK 4 (16 digits: e.g. 0100130010003200) ---
  {
    id: "b4_d1_2",
    code: "B4_01",
    label: "Opciones de Red / Host (d1-2)",
    description: "Par de dígitos 1–2 del Banco 4.",
    bank: 4,
    startDigit: 1,
    length: 2,
    category: "system",
    risk: "caution",
    verified: false,
  },
  {
    id: "b4_d3_4",
    code: "B4_03",
    label: "Opciones de Memoria / PLU (d3-4)",
    description: "Par de dígitos 3–4 del Banco 4.",
    bank: 4,
    startDigit: 3,
    length: 2,
    category: "system",
    risk: "caution",
    verified: false,
  },
  {
    id: "b4_d5_6",
    code: "B4_05",
    label: "Modo de Impresión Avanzado (d5-6)",
    description: "Par de dígitos 5–6 del Banco 4.",
    bank: 4,
    startDigit: 5,
    length: 2,
    category: "printing",
    risk: "normal",
    verified: false,
  },
  {
    id: "b4_d7_8",
    code: "B4_07",
    label: "Formato de Hora / Calendario (d7-8)",
    description: "Par de dígitos 7–8 del Banco 4.",
    bank: 4,
    startDigit: 7,
    length: 2,
    category: "region",
    risk: "normal",
    verified: false,
  },
  {
    id: "b4_d9_10",
    code: "B4_09",
    label: "Opciones de Autoservicio / PoS (d9-10)",
    description: "Par de dígitos 9–10 del Banco 4.",
    bank: 4,
    startDigit: 9,
    length: 2,
    category: "operation",
    risk: "normal",
    verified: false,
  },
  {
    id: "b4_d11_12",
    code: "B4_11",
    label: "Control de Auditoría / Logs (d11-12)",
    description: "Par de dígitos 11–12 del Banco 4.",
    bank: 4,
    startDigit: 11,
    length: 2,
    category: "system",
    risk: "normal",
    verified: false,
  },
  {
    id: "b4_cre",
    code: "CRE",
    label: "País / Región Comercial (CRE)",
    description:
      "Código de país / región para reglas metrológicas y formatos (Banco 4, dígitos 13–14).",
    bank: 4,
    startDigit: 13,
    length: 2,
    category: "region",
    risk: "caution",
    verified: true,
    options: [
      { value: "00", label: "00 — General / Reino Unido" },
      { value: "01", label: "01 — Estados Unidos" },
      { value: "10", label: "10 — España / Europa" },
      { value: "32", label: "32 — México" },
    ],
  },
  {
    id: "b4_d15_16",
    code: "B4_15",
    label: "Reservado / Auxiliar Banco 4 (d15-16)",
    description: "Par de dígitos 15–16 del Banco 4.",
    bank: 4,
    startDigit: 15,
    length: 2,
    category: "system",
    risk: "caution",
    verified: false,
  },
];

export const DEFAULT_BANKS_748: ScaleSettings748 = {
  version: "0001",
  bank1: "8610011020214130",
  bank2: "0020020110111000",
  bank3: "0110110100111000",
  bank4: "0100130010003200",
};

export function getBankString(settings: ScaleSettings748, bank: BankNumber): string {
  const raw =
    bank === 1
      ? settings.bank1
      : bank === 2
        ? settings.bank2
        : bank === 3
          ? settings.bank3
          : settings.bank4;
  return raw.padEnd(16, "0").slice(0, 16);
}

export function readBankFieldValue(settings: ScaleSettings748, field: BankFieldSpec): string {
  const bankStr = getBankString(settings, field.bank);
  const idx = field.startDigit - 1;
  return bankStr.slice(idx, idx + field.length);
}

export function writeBankFieldValue(
  settings: ScaleSettings748,
  field: BankFieldSpec,
  nextValue: string,
): ScaleSettings748 {
  const clean = nextValue.replace(/[^0-9A-Fa-f]/g, "").toUpperCase();
  const padded = clean.padStart(field.length, "0").slice(-field.length);
  const bankStr = getBankString(settings, field.bank);
  const idx = field.startDigit - 1;
  const updatedBank = bankStr.slice(0, idx) + padded + bankStr.slice(idx + field.length);

  return {
    ...settings,
    bank1: field.bank === 1 ? updatedBank : settings.bank1,
    bank2: field.bank === 2 ? updatedBank : settings.bank2,
    bank3: field.bank === 3 ? updatedBank : settings.bank3,
    bank4: field.bank === 4 ? updatedBank : settings.bank4,
  };
}

export interface BankFieldDiff {
  field: BankFieldSpec;
  before: string;
  after: string;
}

export function diffBanks748(
  baseline: ScaleSettings748,
  current: ScaleSettings748,
): BankFieldDiff[] {
  const diffs: BankFieldDiff[] = [];
  for (const field of BANK_748_FIELDS) {
    const before = readBankFieldValue(baseline, field);
    const after = readBankFieldValue(current, field);
    if (before !== after) {
      diffs.push({ field, before, after });
    }
  }
  return diffs;
}
