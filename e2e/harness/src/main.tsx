import "@openscale/ui/theme.css";
import type { OpenScaleTransport, Plu } from "@openscale/client";
import { initOpenScaleI18n } from "@openscale/i18n";
import { Badge, Button, DataTable, OpenScaleProvider } from "@openscale/ui";
import type { ColumnDef } from "@tanstack/react-table";
import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { useTranslation } from "react-i18next";

initOpenScaleI18n("en");

const INITIAL_PLUS: Plu[] = [
  {
    id: "plu-1",
    key: { did: 1, plu: 100 },
    description: "POLLO FRESCO",
    button_text: "Pollo",
    price_1: 4500,
    price_2: 0,
    price_3: 0,
    product_type: "by_weight",
    ean: "2000100000000",
    sku: null,
    group_id: 1,
    barcode_format_id: 1,
    label_format_id: 1,
    tare_id: 0,
    tare_grams: null,
    date_offset_1: 7,
    date_print_control_1: 1,
    date_offset_2: 0,
    date_print_control_2: 0,
    tax_rate_id_1: 0,
    tax_rate_id_2: 0,
    text_areas: [],
    delta_hash: "demo",
    meta: {
      store_id: "01900000-0000-7000-8000-000000000001",
      sync_version: 1,
      updated_at: "2026-11-22T00:00:00Z",
      deleted_at: null,
    },
  },
];

const mockTransport: OpenScaleTransport = {
  discoverScales: async () => [],
  listScales: async () => [],
  upsertScale: async (s) => s,
  listPlus: async () => INITIAL_PLUS,
  getPlu: async () => INITIAL_PLUS[0] ?? null,
  upsertPlu: async (p) => p,
  deletePlu: async () => true,
  subscribe: () => () => undefined,
};

function App() {
  const { t, i18n } = useTranslation();
  const [rows, setRows] = useState<Plu[]>(INITIAL_PLUS);

  const columns: ColumnDef<Plu>[] = [
    { id: "did", header: t("catalog.columns.did"), accessorFn: (row) => row.key.did },
    { id: "plu", header: t("catalog.columns.plu"), accessorFn: (row) => row.key.plu },
    { accessorKey: "description", header: t("catalog.columns.description") },
    {
      id: "price1",
      header: t("catalog.columns.price1"),
      accessorFn: (row) => `$${(row.price_1 / 100).toFixed(2)}`,
    },
  ];

  const toggleLanguage = () => {
    void i18n.changeLanguage(i18n.language === "en" ? "es" : "en");
  };

  const addSamplePlu = () => {
    setRows((prev) => {
      const template = INITIAL_PLUS[0];
      if (!template) {
        return prev;
      }
      return [
        ...prev,
        {
          ...template,
          id: `plu-${prev.length + 1}`,
          key: { did: 1, plu: 100 + prev.length },
          description: "BISTEC DE RES",
          price_1: 18900,
        },
      ];
    });
  };

  return (
    <main className="mx-auto max-w-4xl space-y-4 p-6">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t("catalog.title")}</h1>
          <Badge variant="success">{t("scales.status.online")}</Badge>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={toggleLanguage}>
            {t("common.language")}: {i18n.language.toUpperCase()}
          </Button>
          <Button onClick={addSamplePlu}>{t("catalog.addPlu")}</Button>
        </div>
      </header>
      <DataTable columns={columns} data={rows} emptyMessage={t("common.noData")} />
    </main>
  );
}

const rootElement = document.getElementById("root");
if (rootElement) {
  createRoot(rootElement).render(
    <StrictMode>
      <OpenScaleProvider transport={mockTransport}>
        <App />
      </OpenScaleProvider>
    </StrictMode>,
  );
}
