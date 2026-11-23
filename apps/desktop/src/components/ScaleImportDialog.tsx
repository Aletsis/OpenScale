import type {
  ImportCategory,
  ImportConflict,
  ImportSelection,
  Scale,
  ScaleImportApplyRequest,
  ScaleImportPreview,
  ScaleImportResult,
} from "@openscale/client";
import { Badge, Button, Dialog, useOpenScaleTransport } from "@openscale/ui";
import {
  AlertTriangle,
  Barcode,
  Database,
  Download,
  FileCode2,
  FileText,
  Layers,
  Loader2,
  Percent,
  Weight,
} from "lucide-react";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

type ResolutionChoice = "local" | "scale";
type WizardStep = "select" | "loading" | "conflicts" | "applying";

interface ScaleImportDialogProps {
  open: boolean;
  scale: Scale | null;
  fromSave: boolean;
  onClose: () => void;
  onCompleted: (result: ScaleImportResult, scale: Scale) => void;
}

interface CategoryMeta {
  key: keyof ImportSelection;
  category: ImportCategory;
  xsOnly: boolean;
  icon: ReactNode;
}

const CATEGORIES: CategoryMeta[] = [
  {
    key: "plus",
    category: "plu",
    xsOnly: false,
    icon: <Database className="h-4 w-4 text-primary" />,
  },
  {
    key: "departments",
    category: "department",
    xsOnly: true,
    icon: <Layers className="h-4 w-4 text-primary" />,
  },
  {
    key: "tares",
    category: "tare",
    xsOnly: true,
    icon: <Weight className="h-4 w-4 text-primary" />,
  },
  {
    key: "tax_rates",
    category: "tax_rate",
    xsOnly: true,
    icon: <Percent className="h-4 w-4 text-primary" />,
  },
  {
    key: "stored_texts",
    category: "stored_text",
    xsOnly: true,
    icon: <FileText className="h-4 w-4 text-primary" />,
  },
  {
    key: "barcode_formats",
    category: "barcode_format",
    xsOnly: true,
    icon: <Barcode className="h-4 w-4 text-primary" />,
  },
  {
    key: "label_formats",
    category: "label_format",
    xsOnly: true,
    icon: <FileCode2 className="h-4 w-4 text-primary" />,
  },
];

function conflictId(conflict: Pick<ImportConflict, "category" | "key">): string {
  return `${conflict.category}::${conflict.key}`;
}

function defaultSelectionForScale(scale: Scale | null): ImportSelection {
  const isXs = scale?.family === "avery_berkel_xs";
  return {
    plus: true,
    departments: isXs,
    tares: isXs,
    tax_rates: isXs,
    stored_texts: isXs,
    barcode_formats: isXs,
    label_formats: isXs,
  };
}

function countUnchanged(preview: ScaleImportPreview): number {
  const c = preview.unchanged_counts;
  return (
    c.plus +
    c.departments +
    c.tares +
    c.tax_rates +
    c.stored_texts +
    c.barcode_formats +
    c.label_formats
  );
}

function countNew(preview: ScaleImportPreview): number {
  return (
    preview.new_plus.length +
    preview.new_departments.length +
    preview.new_tares.length +
    preview.new_tax_rates.length +
    preview.new_stored_texts.length +
    preview.new_barcode_formats.length +
    preview.new_label_formats.length
  );
}

function buildApplyRequest(
  preview: ScaleImportPreview,
  resolutions: Record<string, ResolutionChoice>,
): ScaleImportApplyRequest {
  const keepScale = (category: ImportCategory, key: string) =>
    resolutions[`${category}::${key}`] === "scale";

  const keptLocalConflicts = preview.conflicts.filter(
    (c) => resolutions[conflictId(c)] !== "scale",
  ).length;

  return {
    scale_id: preview.scale_id,
    plus: [
      ...preview.new_plus,
      ...preview.conflict_plus.filter((p) => keepScale("plu", `${p.key.did}:${p.key.plu}`)),
    ],
    departments: [
      ...preview.new_departments,
      ...preview.conflict_departments.filter((d) => keepScale("department", String(d.did))),
    ],
    tares: [
      ...preview.new_tares,
      ...preview.conflict_tares.filter((t) => keepScale("tare", String(t.cid))),
    ],
    tax_rates: [
      ...preview.new_tax_rates,
      ...preview.conflict_tax_rates.filter((t) => keepScale("tax_rate", String(t.tax_id))),
    ],
    stored_texts: [
      ...preview.new_stored_texts,
      ...preview.conflict_stored_texts.filter((t) => keepScale("stored_text", String(t.text_id))),
    ],
    barcode_formats: [
      ...preview.new_barcode_formats,
      ...preview.conflict_barcode_formats.filter((b) => keepScale("barcode_format", String(b.bid))),
    ],
    label_formats: [
      ...preview.new_label_formats,
      ...preview.conflict_label_formats.filter((l) => keepScale("label_format", String(l.pfid))),
    ],
    kept_local_conflicts: keptLocalConflicts,
    skipped_unchanged: countUnchanged(preview),
  };
}

export function ScaleImportDialog({
  open,
  scale,
  fromSave,
  onClose,
  onCompleted,
}: ScaleImportDialogProps) {
  const { t } = useTranslation();
  const transport = useOpenScaleTransport();

  const [step, setStep] = useState<WizardStep>("select");
  const [selection, setSelection] = useState<ImportSelection>(() =>
    defaultSelectionForScale(scale),
  );
  const [preview, setPreview] = useState<ScaleImportPreview | null>(null);
  const [resolutions, setResolutions] = useState<Record<string, ResolutionChoice>>({});
  const [categoryFilter, setCategoryFilter] = useState<ImportCategory | "all">("all");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (open && scale) {
      setStep("select");
      setSelection(defaultSelectionForScale(scale));
      setPreview(null);
      setResolutions({});
      setCategoryFilter("all");
      setErrorMessage(null);
    }
  }, [open, scale]);

  const isXs = scale?.family === "avery_berkel_xs";

  const hasAnySelected = useMemo(() => Object.values(selection).some(Boolean), [selection]);

  const conflictCategoriesPresent = useMemo(() => {
    if (!preview) {
      return [];
    }
    const counts = new Map<ImportCategory, number>();
    for (const c of preview.conflicts) {
      counts.set(c.category, (counts.get(c.category) ?? 0) + 1);
    }
    return Array.from(counts.entries());
  }, [preview]);

  const filteredConflicts = useMemo(() => {
    if (!preview) {
      return [];
    }
    if (categoryFilter === "all") {
      return preview.conflicts;
    }
    return preview.conflicts.filter((c) => c.category === categoryFilter);
  }, [preview, categoryFilter]);

  const scaleChoiceCount = useMemo(() => {
    if (!preview) {
      return 0;
    }
    return preview.conflicts.filter((c) => resolutions[conflictId(c)] === "scale").length;
  }, [preview, resolutions]);

  const localChoiceCount = (preview?.conflicts.length ?? 0) - scaleChoiceCount;

  if (!scale) {
    return null;
  }

  const toggleAll = (checked: boolean) => {
    setSelection({
      plus: checked,
      departments: checked && isXs,
      tares: checked && isXs,
      tax_rates: checked && isXs,
      stored_texts: checked && isXs,
      barcode_formats: checked && isXs,
      label_formats: checked && isXs,
    });
  };

  const handleStartPreview = async () => {
    setErrorMessage(null);
    setStep("loading");
    try {
      const nextPreview = await transport.previewImportFromScale(scale.id, selection);
      setPreview(nextPreview);

      if (nextPreview.conflicts.length === 0) {
        setStep("applying");
        const request = buildApplyRequest(nextPreview, {});
        const result = await transport.applyScaleImport(request);
        onCompleted(result, scale);
        onClose();
        return;
      }

      const initialResolutions: Record<string, ResolutionChoice> = {};
      for (const conflict of nextPreview.conflicts) {
        initialResolutions[conflictId(conflict)] = "local";
      }
      setResolutions(initialResolutions);
      setCategoryFilter("all");
      setStep("conflicts");
    } catch (err) {
      setErrorMessage(String(err));
      setStep("select");
    }
  };

  const handleApplyResolved = async () => {
    if (!preview) {
      return;
    }
    setErrorMessage(null);
    setStep("applying");
    try {
      const request = buildApplyRequest(preview, resolutions);
      const result = await transport.applyScaleImport(request);
      onCompleted(result, scale);
      onClose();
    } catch (err) {
      setErrorMessage(String(err));
      setStep("conflicts");
    }
  };

  const setAllResolutions = (choice: ResolutionChoice) => {
    if (!preview) {
      return;
    }
    const next: Record<string, ResolutionChoice> = {};
    for (const conflict of preview.conflicts) {
      next[conflictId(conflict)] = choice;
    }
    setResolutions(next);
  };

  const isBusy = step === "loading" || step === "applying";

  const dialogTitle =
    step === "conflicts"
      ? t("scales.importDialog.conflictsTitle")
      : fromSave
        ? t("scales.importDialog.titleAfterSave")
        : t("scales.importDialog.titleManual");

  const dialogDescription =
    step === "conflicts"
      ? t("scales.importDialog.conflictsDescription")
      : fromSave
        ? t("scales.importDialog.descriptionAfterSave", {
            name: scale.name,
            ip: scale.ip_address,
          })
        : t("scales.importDialog.descriptionManual", {
            name: scale.name,
            ip: scale.ip_address,
          });

  const renderFooter = () => {
    if (step === "select") {
      return (
        <>
          <Button size="sm" variant="ghost" onClick={onClose}>
            {fromSave ? t("scales.importDialog.skipButton") : t("common.cancel")}
          </Button>
          <Button size="sm" disabled={!hasAnySelected} onClick={() => void handleStartPreview()}>
            <Download className="h-3.5 w-3.5" />
            {t("scales.importDialog.startButton")}
          </Button>
        </>
      );
    }

    if (step === "conflicts") {
      return (
        <div className="flex w-full flex-wrap items-center justify-between gap-2">
          <span className="text-xs text-muted-foreground tabular-nums">
            {t("scales.importDialog.resolutionCount", {
              scale: scaleChoiceCount,
              local: localChoiceCount,
            })}
          </span>
          <div className="flex items-center gap-2">
            <Button size="sm" variant="ghost" onClick={onClose}>
              {t("common.cancel")}
            </Button>
            <Button size="sm" onClick={() => void handleApplyResolved()}>
              {t("scales.importDialog.applyButton")}
            </Button>
          </div>
        </div>
      );
    }

    return null;
  };

  return (
    <Dialog
      open={open}
      onClose={() => {
        if (!isBusy) {
          onClose();
        }
      }}
      title={dialogTitle}
      description={dialogDescription}
      size={step === "conflicts" ? "lg" : "md"}
      closeLabel={t("common.close")}
      footer={renderFooter()}
    >
      {errorMessage && (
        <div className="mb-4 flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <div className="min-w-0 flex-1 break-words">{errorMessage}</div>
        </div>
      )}

      {step === "select" && (
        <div className="space-y-3">
          <div className="flex items-center justify-between text-xs">
            <Badge variant="default">
              {isXs ? "XS / M-Series" : "XTi / XTRA"} · {scale.ip_address}
            </Badge>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => toggleAll(true)}
                className="cursor-pointer font-medium text-primary hover:underline"
              >
                {t("scales.importDialog.selectAll")}
              </button>
              <span className="text-muted-foreground">·</span>
              <button
                type="button"
                onClick={() => toggleAll(false)}
                className="cursor-pointer font-medium text-muted-foreground hover:text-foreground hover:underline"
              >
                {t("scales.importDialog.selectNone")}
              </button>
            </div>
          </div>

          <div className="space-y-2">
            {CATEGORIES.map((item) => {
              const disabled = item.xsOnly && !isXs;
              const checked = selection[item.key];
              return (
                <label
                  key={item.key}
                  className={
                    disabled
                      ? "flex cursor-not-allowed items-start gap-3 rounded-lg border border-border/60 bg-muted/30 p-3 opacity-60"
                      : "flex cursor-pointer items-start gap-3 rounded-lg border border-border bg-background p-3 transition-colors hover:border-primary/50 hover:bg-muted/30"
                  }
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={disabled}
                    onChange={(ev) =>
                      setSelection((prev) => ({
                        ...prev,
                        [item.key]: ev.target.checked,
                      }))
                    }
                    className="mt-0.5 h-4 w-4 rounded border-border accent-primary"
                  />
                  <div className="mt-0.5 shrink-0">{item.icon}</div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium">
                        {t(`scales.importDialog.categories.${item.category}`)}
                      </span>
                      {disabled && (
                        <Badge variant="default">{t("scales.importDialog.xsOnly")}</Badge>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground">
                      {t(`scales.importDialog.categories.${item.category}Desc`)}
                    </p>
                  </div>
                </label>
              );
            })}
          </div>
        </div>
      )}

      {(step === "loading" || step === "applying") && (
        <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
          <Loader2 className="h-8 w-8 animate-spin text-primary" />
          <p className="text-sm font-medium">
            {step === "loading"
              ? t("scales.importDialog.reading", { name: scale.name, ip: scale.ip_address })
              : t("scales.importDialog.applying")}
          </p>
          {step === "loading" && (
            <p className="max-w-sm text-xs text-muted-foreground">
              {t("scales.importDialog.readingHint")}
            </p>
          )}
        </div>
      )}

      {step === "conflicts" && preview && (
        <div className="space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="success">
                {t("scales.importDialog.summaryNew", { count: countNew(preview) })}
              </Badge>
              <Badge variant="default">
                {t("scales.importDialog.summaryUnchanged", { count: countUnchanged(preview) })}
              </Badge>
              <Badge variant="warning">
                {t("scales.importDialog.summaryConflicts", { count: preview.conflicts.length })}
              </Badge>
            </div>
            <div className="flex flex-wrap items-center gap-1.5">
              <Button size="sm" variant="outline" onClick={() => setAllResolutions("local")}>
                {t("scales.importDialog.keepAllLocal")}
              </Button>
              <Button size="sm" variant="outline" onClick={() => setAllResolutions("scale")}>
                {t("scales.importDialog.keepAllScale")}
              </Button>
            </div>
          </div>

          {conflictCategoriesPresent.length > 1 && (
            <div className="flex flex-wrap gap-1.5">
              <Button
                size="sm"
                variant={categoryFilter === "all" ? "default" : "ghost"}
                onClick={() => setCategoryFilter("all")}
              >
                {t("scales.importDialog.allCategories")} ({preview.conflicts.length})
              </Button>
              {conflictCategoriesPresent.map(([cat, count]) => (
                <Button
                  key={cat}
                  size="sm"
                  variant={categoryFilter === cat ? "default" : "ghost"}
                  onClick={() => setCategoryFilter(cat)}
                >
                  {t(`scales.importDialog.categories.${cat}`)} ({count})
                </Button>
              ))}
            </div>
          )}

          <div className="max-h-[45vh] space-y-3 overflow-y-auto pr-1">
            {filteredConflicts.map((conflict) => {
              const id = conflictId(conflict);
              const choice = resolutions[id] ?? "local";
              return (
                <div
                  key={id}
                  className="rounded-lg border border-border bg-background p-3 shadow-xs"
                >
                  <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2">
                      <Badge variant="default">
                        {t(`scales.importDialog.categories.${conflict.category}`)}
                      </Badge>
                      <span className="truncate text-sm font-semibold">{conflict.label}</span>
                    </div>
                    <div className="flex items-center gap-1 rounded-md bg-muted p-0.5">
                      <button
                        type="button"
                        onClick={() =>
                          setResolutions((prev) => ({
                            ...prev,
                            [id]: "local",
                          }))
                        }
                        className={
                          choice === "local"
                            ? "cursor-pointer rounded px-2.5 py-1 text-xs font-medium bg-card text-foreground shadow-xs"
                            : "cursor-pointer rounded px-2.5 py-1 text-xs font-medium text-muted-foreground hover:text-foreground"
                        }
                      >
                        {t("scales.importDialog.keepLocal")}
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          setResolutions((prev) => ({
                            ...prev,
                            [id]: "scale",
                          }))
                        }
                        className={
                          choice === "scale"
                            ? "cursor-pointer rounded px-2.5 py-1 text-xs font-medium bg-primary text-primary-foreground shadow-xs"
                            : "cursor-pointer rounded px-2.5 py-1 text-xs font-medium text-muted-foreground hover:text-foreground"
                        }
                      >
                        {t("scales.importDialog.useScale")}
                      </button>
                    </div>
                  </div>

                  <div className="overflow-x-auto rounded border border-border/70">
                    <table className="w-full text-left text-xs">
                      <thead className="bg-muted/50 text-muted-foreground">
                        <tr>
                          <th className="px-2.5 py-1.5 font-medium">
                            {t("scales.importDialog.colField")}
                          </th>
                          <th className="px-2.5 py-1.5 font-medium">
                            {t("scales.importDialog.colLocal")}
                          </th>
                          <th className="px-2.5 py-1.5 font-medium">
                            {t("scales.importDialog.colScale")}
                          </th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-border/60">
                        {conflict.diffs.map((diff) => (
                          <tr key={diff.field}>
                            <td className="px-2.5 py-1.5 font-medium text-muted-foreground">
                              {t(`scales.importDialog.fields.${diff.field}`, {
                                defaultValue: diff.field,
                              })}
                            </td>
                            <td
                              className={
                                choice === "local"
                                  ? "bg-emerald-500/10 px-2.5 py-1.5 font-mono text-foreground"
                                  : "px-2.5 py-1.5 font-mono text-muted-foreground line-through"
                              }
                            >
                              {diff.local_value || "—"}
                            </td>
                            <td
                              className={
                                choice === "scale"
                                  ? "bg-primary/10 px-2.5 py-1.5 font-mono text-foreground"
                                  : "px-2.5 py-1.5 font-mono text-muted-foreground"
                              }
                            >
                              {diff.remote_value || "—"}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </Dialog>
  );
}
