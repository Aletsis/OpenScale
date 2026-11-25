import {
  BANK_748_FIELDS,
  BANK_CATEGORIES,
  type BankCategory,
  type BankFieldSpec,
  type BankNumber,
  type ScaleSettings748,
  diffBanks748,
  getBankString,
  readBankFieldValue,
  writeBankFieldValue,
} from "@openscale/client";
import { Badge, Button } from "@openscale/ui";
import { AlertTriangle, CheckCircle2, Download, Search, ShieldAlert } from "lucide-react";
import { useMemo, useState } from "react";

interface ScaleBanksEditorProps {
  value: ScaleSettings748;
  baseline?: ScaleSettings748 | null;
  onChange: (next: ScaleSettings748) => void;
  readOnly?: boolean;
}

export function ScaleBanksEditor({
  value,
  baseline,
  onChange,
  readOnly = false,
}: ScaleBanksEditorProps) {
  const [activeBank, setActiveBank] = useState<BankNumber | "all">(1);
  const [activeCategory, setActiveCategory] = useState<BankCategory | "all">("all");
  const [search, setSearch] = useState("");
  const [selectedFieldId, setSelectedFieldId] = useState<string | null>(null);

  const diffs = useMemo(() => (baseline ? diffBanks748(baseline, value) : []), [baseline, value]);
  const diffSet = useMemo(() => new Set(diffs.map((d) => d.field.id)), [diffs]);

  const filteredFields = useMemo(() => {
    const q = search.trim().toLowerCase();
    return BANK_748_FIELDS.filter((f) => {
      if (activeBank !== "all" && f.bank !== activeBank) {
        return false;
      }
      if (activeCategory !== "all" && f.category !== activeCategory) {
        return false;
      }
      if (!q) {
        return true;
      }
      return (
        f.code.toLowerCase().includes(q) ||
        f.label.toLowerCase().includes(q) ||
        f.description.toLowerCase().includes(q)
      );
    });
  }, [activeBank, activeCategory, search]);

  const handleExportBackup = () => {
    const blob = new Blob([JSON.stringify(value, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `scale-banks-748-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const renderBankMatrixRow = (bank: BankNumber) => {
    const bankStr = getBankString(value, bank);
    const baseStr = baseline ? getBankString(baseline, bank) : null;
    const fieldsInBank = BANK_748_FIELDS.filter((f) => f.bank === bank);

    return (
      <div key={bank} className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => setActiveBank(bank)}
          className={`w-20 cursor-pointer rounded px-2 py-1 text-left font-mono text-xs font-semibold ${
            activeBank === bank
              ? "bg-primary text-primary-foreground"
              : "bg-muted text-muted-foreground hover:text-foreground"
          }`}
        >
          BANK {bank}
        </button>
        <div className="flex flex-wrap gap-1">
          {fieldsInBank.map((field) => {
            const val = readBankFieldValue(value, field);
            const changed = diffSet.has(field.id);
            const isSelected = selectedFieldId === field.id;
            const digitRange =
              field.length === 1
                ? `d${field.startDigit}`
                : `d${field.startDigit}-${field.startDigit + field.length - 1}`;
            return (
              <button
                key={field.id}
                type="button"
                title={`${field.code} (${digitRange}): ${field.label} = ${val}`}
                onClick={() => {
                  setActiveBank(bank);
                  setActiveCategory("all");
                  setSelectedFieldId(field.id);
                }}
                className={`flex cursor-pointer flex-col items-center rounded border px-1.5 py-0.5 font-mono transition-colors ${
                  isSelected
                    ? "border-primary bg-primary/15 ring-1 ring-primary"
                    : changed
                      ? "border-amber-500 bg-amber-500/15 text-amber-800 dark:text-amber-200"
                      : "border-border bg-background hover:bg-muted/60"
                }`}
              >
                <span className="text-[9px] leading-tight text-muted-foreground">{field.code}</span>
                <span className="text-xs font-bold leading-tight">{val}</span>
              </button>
            );
          })}
        </div>
        {baseStr && baseStr !== bankStr && (
          <Badge variant="warning" className="text-[10px]">
            Modificado
          </Badge>
        )}
      </div>
    );
  };

  const renderFieldControl = (field: BankFieldSpec) => {
    const currentVal = readBankFieldValue(value, field);
    const baseVal = baseline ? readBankFieldValue(baseline, field) : null;
    const isChanged = baseVal !== null && baseVal !== currentVal;
    const hasCustomOption = field.options && !field.options.some((opt) => opt.value === currentVal);
    const digitRange =
      field.length === 1
        ? `Dígito ${field.startDigit}`
        : `Dígitos ${field.startDigit}–${field.startDigit + field.length - 1}`;

    return (
      <div
        key={field.id}
        className={`flex flex-col justify-between gap-2 rounded-md border p-3 text-xs transition-colors ${
          selectedFieldId === field.id
            ? "border-primary bg-primary/5 ring-1 ring-primary/40"
            : isChanged
              ? "border-amber-500/60 bg-amber-500/5"
              : "border-border bg-background"
        }`}
      >
        <div className="space-y-1">
          <div className="flex flex-wrap items-center justify-between gap-1">
            <div className="flex items-center gap-1.5">
              <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-[10px] font-bold text-primary">
                B{field.bank} · {digitRange}
              </span>
              <span className="font-mono font-semibold">{field.code}</span>
            </div>
            <div className="flex items-center gap-1">
              {field.verified && (
                <Badge variant="success" className="px-1.5 py-0 text-[10px]">
                  Verificado
                </Badge>
              )}
              {field.risk === "calibration" && (
                <Badge variant="danger" className="gap-0.5 px-1.5 py-0 text-[10px]">
                  <ShieldAlert className="h-2.5 w-2.5" /> Metrología
                </Badge>
              )}
              {field.risk === "caution" && (
                <Badge variant="warning" className="px-1.5 py-0 text-[10px]">
                  Precaución
                </Badge>
              )}
            </div>
          </div>
          <div className="font-medium text-foreground">{field.label}</div>
          <p className="text-[11px] text-muted-foreground">{field.description}</p>
        </div>

        <div className="flex items-center gap-2 pt-1">
          {field.options ? (
            <select
              disabled={readOnly}
              value={currentVal}
              onChange={(ev) => onChange(writeBankFieldValue(value, field, ev.target.value))}
              className="w-full rounded border border-border bg-card px-2 py-1 font-mono text-xs"
            >
              {hasCustomOption && (
                <option value={currentVal}>{currentVal} — Valor actual de báscula</option>
              )}
              {field.options.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          ) : (
            <input
              type="text"
              disabled={readOnly}
              maxLength={field.length}
              value={currentVal}
              onChange={(ev) => onChange(writeBankFieldValue(value, field, ev.target.value))}
              className="w-24 rounded border border-border bg-card px-2 py-1 text-center font-mono text-xs font-bold tracking-widest"
            />
          )}
          {isChanged && (
            <span className="shrink-0 font-mono text-[10px] text-amber-600 dark:text-amber-400">
              antes: {baseVal}
            </span>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="space-y-4">
      {/* MxBusiness-style 4-Bank Matrix */}
      <div className="space-y-2 rounded-md border border-border bg-muted/20 p-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Matriz de Bancos #748 (v{value.version})
            </span>
            {diffs.length > 0 ? (
              <Badge variant="warning" className="gap-1">
                <AlertTriangle className="h-3 w-3" /> {diffs.length} parámetro(s) modificado(s)
              </Badge>
            ) : (
              <Badge variant="success" className="gap-1">
                <CheckCircle2 className="h-3 w-3" /> Sin diferencias
              </Badge>
            )}
          </div>
          <Button size="sm" variant="outline" onClick={handleExportBackup}>
            <Download className="h-3.5 w-3.5" /> Respaldar JSON
          </Button>
        </div>

        <div className="space-y-1.5">
          {([1, 2, 3, 4] as const).map((b) => renderBankMatrixRow(b))}
        </div>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap gap-1">
          <Button
            size="sm"
            variant={activeBank === "all" ? "default" : "outline"}
            onClick={() => setActiveBank("all")}
          >
            Todos
          </Button>
          {([1, 2, 3, 4] as const).map((b) => (
            <Button
              key={`tab-b${b}`}
              size="sm"
              variant={activeBank === b ? "default" : "outline"}
              onClick={() => setActiveBank(b)}
            >
              Banco {b}
            </Button>
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <select
            value={activeCategory}
            onChange={(ev) => setActiveCategory(ev.target.value as BankCategory | "all")}
            className="rounded border border-border bg-background px-2 py-1 text-xs"
          >
            <option value="all">Todas las categorías</option>
            {BANK_CATEGORIES.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
          <div className="relative w-52">
            <Search className="pointer-events-none absolute top-1/2 left-2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <input
              type="search"
              value={search}
              onChange={(ev) => setSearch(ev.target.value)}
              placeholder="Buscar parámetro..."
              className="w-full rounded border border-border bg-background py-1 pr-2 pl-7 text-xs"
            />
          </div>
        </div>
      </div>

      {/* Parameter Cards */}
      <div className="grid grid-cols-1 gap-2.5 md:grid-cols-2 lg:grid-cols-3">
        {filteredFields.map(renderFieldControl)}
      </div>
    </div>
  );
}
