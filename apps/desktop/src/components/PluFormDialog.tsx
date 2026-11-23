import type { Plu, ProductType } from "@openscale/client";
import { Button, Dialog } from "@openscale/ui";
import { AlertTriangle, Barcode, DollarSign, Tag } from "lucide-react";
import { type FormEvent, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

export interface PluFormState {
  did: string;
  plu: string;
  description: string;
  buttonText: string;
  price1: string;
  price2: string;
  productType: ProductType;
  ean: string;
  sku: string;
  tareGrams: string;
  dateOffset1: string;
  labelFormatId: string;
  barcodeFormatId: string;
}

const INPUT_CLASS =
  "w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm text-foreground transition-colors placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20";

function buildInitialState(editingPlu: Plu | null, existingPlus: Plu[]): PluFormState {
  if (editingPlu) {
    return {
      did: String(editingPlu.key.did),
      plu: String(editingPlu.key.plu),
      description: editingPlu.description,
      buttonText: editingPlu.button_text ?? "",
      price1: (editingPlu.price_1 / 100).toFixed(2),
      price2: (editingPlu.price_2 / 100).toFixed(2),
      productType: editingPlu.product_type,
      ean: editingPlu.ean ?? "",
      sku: editingPlu.sku ?? "",
      tareGrams: editingPlu.tare_grams !== null ? String(editingPlu.tare_grams) : "",
      dateOffset1: String(editingPlu.date_offset_1),
      labelFormatId: String(editingPlu.label_format_id),
      barcodeFormatId: String(editingPlu.barcode_format_id),
    };
  }

  const maxPlu = existingPlus.reduce((max, item) => Math.max(max, item.key.plu), 0);
  return {
    did: "1",
    plu: String(maxPlu + 1),
    description: "",
    buttonText: "",
    price1: "0.00",
    price2: "0.00",
    productType: "by_weight",
    ean: "",
    sku: "",
    tareGrams: "",
    dateOffset1: "0",
    labelFormatId: "1",
    barcodeFormatId: "1",
  };
}

export interface PluFormDialogProps {
  open: boolean;
  editingPlu: Plu | null;
  existingPlus: Plu[];
  storeId: string;
  isSaving: boolean;
  onClose: () => void;
  onInvalidKey: () => void;
  onSubmit: (record: Plu) => Promise<void> | void;
}

export function PluFormDialog({
  open,
  editingPlu,
  existingPlus,
  storeId,
  isSaving,
  onClose,
  onInvalidKey,
  onSubmit,
}: PluFormDialogProps) {
  const { t } = useTranslation();
  const [form, setForm] = useState<PluFormState>(() => buildInitialState(editingPlu, existingPlus));

  useEffect(() => {
    if (open) {
      setForm(buildInitialState(editingPlu, existingPlus));
    }
  }, [open, editingPlu, existingPlus]);

  const duplicatePlu = useMemo(() => {
    const did = Number.parseInt(form.did, 10);
    const pluNum = Number.parseInt(form.plu, 10);
    if (!Number.isFinite(did) || !Number.isFinite(pluNum)) {
      return null;
    }
    return (
      existingPlus.find(
        (item) =>
          item.key.did === did &&
          item.key.plu === pluNum &&
          (!editingPlu || item.id !== editingPlu.id),
      ) ?? null
    );
  }, [form.did, form.plu, existingPlus, editingPlu]);

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const did = Number.parseInt(form.did, 10);
    const pluNum = Number.parseInt(form.plu, 10);
    if (!Number.isFinite(did) || did < 0 || !Number.isFinite(pluNum) || pluNum <= 0) {
      onInvalidKey();
      return;
    }

    const now = new Date().toISOString();
    const dateOffset1 = Math.max(0, Number.parseInt(form.dateOffset1 || "0", 10) || 0);
    const tareVal = form.tareGrams.trim() ? Number.parseInt(form.tareGrams, 10) : null;
    const labelFormatId = Math.max(1, Number.parseInt(form.labelFormatId || "1", 10) || 1);
    const barcodeFormatId = Math.max(1, Number.parseInt(form.barcodeFormatId || "1", 10) || 1);

    const record: Plu = {
      id: editingPlu?.id ?? duplicatePlu?.id ?? crypto.randomUUID(),
      key: { did, plu: pluNum },
      description: form.description.trim(),
      button_text: form.buttonText.trim() || null,
      price_1: Math.max(0, Math.round((Number.parseFloat(form.price1) || 0) * 100)),
      price_2: Math.max(0, Math.round((Number.parseFloat(form.price2) || 0) * 100)),
      price_3: editingPlu?.price_3 ?? 0,
      product_type: form.productType,
      ean: form.ean.trim() || null,
      sku: form.sku.trim() || null,
      group_id: editingPlu?.group_id ?? 0,
      barcode_format_id: barcodeFormatId,
      label_format_id: labelFormatId,
      tare_id: editingPlu?.tare_id ?? null,
      tare_grams: tareVal !== null && Number.isFinite(tareVal) ? Math.max(0, tareVal) : null,
      date_offset_1: dateOffset1,
      date_print_control_1: dateOffset1 > 0 ? 1 : 0,
      date_offset_2: editingPlu?.date_offset_2 ?? 0,
      date_print_control_2: editingPlu?.date_print_control_2 ?? 0,
      tax_rate_id_1: editingPlu?.tax_rate_id_1 ?? 0,
      tax_rate_id_2: editingPlu?.tax_rate_id_2 ?? 0,
      text_areas: editingPlu?.text_areas ?? [],
      delta_hash: editingPlu?.delta_hash ?? "",
      meta: {
        store_id: editingPlu?.meta.store_id ?? storeId,
        sync_version: (editingPlu?.meta.sync_version ?? 0) + 1,
        updated_at: now,
        deleted_at: null,
      },
    };

    await onSubmit(record);
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={editingPlu ? t("catalog.editPlu") : t("catalog.addPlu")}
      description={
        editingPlu
          ? t("catalog.editDescription", {
              did: editingPlu.key.did,
              plu: editingPlu.key.plu,
            })
          : t("catalog.createDescription")
      }
      size="lg"
      closeLabel={t("common.close")}
      footer={
        <>
          <Button size="sm" variant="ghost" onClick={onClose} disabled={isSaving}>
            {t("common.cancel")}
          </Button>
          <Button size="sm" type="submit" form="plu-form" disabled={isSaving}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <form id="plu-form" onSubmit={(ev) => void handleSubmit(ev)} className="space-y-5 text-sm">
        {duplicatePlu && (
          <div
            role="alert"
            className="flex items-start gap-2.5 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-800 dark:text-amber-300"
          >
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
            <span>
              {t("catalog.form.duplicateWarning", {
                did: duplicatePlu.key.did,
                plu: duplicatePlu.key.plu,
                description: duplicatePlu.description,
              })}
            </span>
          </div>
        )}

        <section className="space-y-3">
          <div className="flex items-center gap-1.5 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
            <Tag className="h-3.5 w-3.5" aria-hidden="true" />
            <span>{t("catalog.form.sectionIdentity")}</span>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-6">
            <label className="flex flex-col gap-1 sm:col-span-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.columns.did")}
              </span>
              <input
                type="number"
                min={0}
                required
                value={form.did}
                onChange={(ev) => setForm({ ...form, did: ev.target.value })}
                className={`${INPUT_CLASS} font-mono tabular-nums`}
              />
            </label>
            <label className="flex flex-col gap-1 sm:col-span-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.columns.plu")}
              </span>
              <input
                type="number"
                min={1}
                required
                value={form.plu}
                onChange={(ev) => setForm({ ...form, plu: ev.target.value })}
                className={`${INPUT_CLASS} font-mono tabular-nums`}
              />
            </label>
            <label className="flex flex-col gap-1 sm:col-span-3">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.columns.description")}
              </span>
              <input
                type="text"
                required
                value={form.description}
                onChange={(ev) => setForm({ ...form, description: ev.target.value })}
                className={INPUT_CLASS}
              />
            </label>
            <label className="flex flex-col gap-1 sm:col-span-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.buttonText")}
              </span>
              <input
                type="text"
                placeholder={t("catalog.form.buttonTextPlaceholder")}
                value={form.buttonText}
                onChange={(ev) => setForm({ ...form, buttonText: ev.target.value })}
                className={INPUT_CLASS}
              />
            </label>
            <label className="flex flex-col gap-1 sm:col-span-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.columns.ean")}
              </span>
              <input
                type="text"
                value={form.ean}
                onChange={(ev) => setForm({ ...form, ean: ev.target.value })}
                className={`${INPUT_CLASS} font-mono tabular-nums`}
              />
            </label>
            <label className="flex flex-col gap-1 sm:col-span-2">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.sku")}
              </span>
              <input
                type="text"
                value={form.sku}
                onChange={(ev) => setForm({ ...form, sku: ev.target.value })}
                className={`${INPUT_CLASS} font-mono`}
              />
            </label>
          </div>
        </section>

        <section className="space-y-3 border-t border-border pt-4">
          <div className="flex items-center gap-1.5 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
            <DollarSign className="h-3.5 w-3.5" aria-hidden="true" />
            <span>{t("catalog.form.sectionPricing")}</span>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.price1")}
              </span>
              <div className="relative">
                <span className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-xs text-muted-foreground">
                  $
                </span>
                <input
                  type="number"
                  step="0.01"
                  min={0}
                  required
                  value={form.price1}
                  onChange={(ev) => setForm({ ...form, price1: ev.target.value })}
                  className={`${INPUT_CLASS} pl-7 font-mono tabular-nums`}
                />
              </div>
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.price2")}
              </span>
              <div className="relative">
                <span className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-xs text-muted-foreground">
                  $
                </span>
                <input
                  type="number"
                  step="0.01"
                  min={0}
                  value={form.price2}
                  onChange={(ev) => setForm({ ...form, price2: ev.target.value })}
                  className={`${INPUT_CLASS} pl-7 font-mono tabular-nums`}
                />
              </div>
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.columns.productType")}
              </span>
              <select
                value={form.productType}
                onChange={(ev) => setForm({ ...form, productType: ev.target.value as ProductType })}
                className={INPUT_CLASS}
              >
                <option value="by_weight">{t("catalog.form.byWeight")}</option>
                <option value="by_count">{t("catalog.form.byCount")}</option>
                <option value="fixed_weight">{t("catalog.form.fixedWeight")}</option>
                <option value="fixed_price">{t("catalog.form.fixedPrice")}</option>
                <option value="by_volume">{t("catalog.form.byVolume")}</option>
                <option value="manual_weight">{t("catalog.form.manualWeight")}</option>
                <option value="negative_by_count">{t("catalog.form.negativeByCount")}</option>
              </select>
            </label>
          </div>
        </section>

        <section className="space-y-3 border-t border-border pt-4">
          <div className="flex items-center gap-1.5 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
            <Barcode className="h-3.5 w-3.5" aria-hidden="true" />
            <span>{t("catalog.form.sectionLabeling")}</span>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-4">
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.tareGrams")}
              </span>
              <input
                type="number"
                min={0}
                value={form.tareGrams}
                onChange={(ev) => setForm({ ...form, tareGrams: ev.target.value })}
                className={`${INPUT_CLASS} font-mono tabular-nums`}
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.shelfLifeDays")}
              </span>
              <input
                type="number"
                min={0}
                value={form.dateOffset1}
                onChange={(ev) => setForm({ ...form, dateOffset1: ev.target.value })}
                className={`${INPUT_CLASS} font-mono tabular-nums`}
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.labelFormatId")}
              </span>
              <input
                type="number"
                min={1}
                value={form.labelFormatId}
                onChange={(ev) => setForm({ ...form, labelFormatId: ev.target.value })}
                className={`${INPUT_CLASS} font-mono tabular-nums`}
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                {t("catalog.form.barcodeFormatId")}
              </span>
              <input
                type="number"
                min={1}
                value={form.barcodeFormatId}
                onChange={(ev) => setForm({ ...form, barcodeFormatId: ev.target.value })}
                className={`${INPUT_CLASS} font-mono tabular-nums`}
              />
            </label>
          </div>
        </section>
      </form>
    </Dialog>
  );
}
