import type {
  BarcodeFormat,
  Department,
  Scale,
  StoredText,
  Tare,
  TaxRate,
} from "@openscale/client";
import { Button, DataTable } from "@openscale/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-react";
import { useState } from "react";

const DEFAULT_STORE_ID = "01900000-0000-7000-8000-000000000001";

type SubTab = "departments" | "tares" | "taxes" | "texts" | "barcodes";

interface MasterTablesViewProps {
  scales: Scale[];
  onStatus: (msg: string) => void;
}

function makeMeta() {
  return {
    store_id: DEFAULT_STORE_ID,
    sync_version: 1,
    updated_at: new Date().toISOString(),
    deleted_at: null,
  };
}

export function MasterTablesView({ scales, onStatus }: MasterTablesViewProps) {
  const queryClient = useQueryClient();
  const [subTab, setSubTab] = useState<SubTab>("departments");
  const [selectedScaleId, setSelectedScaleId] = useState<string>(scales[0]?.id ?? "");

  // Inline quick-add states
  const [idInput, setIdInput] = useState("1");
  const [nameInput, setNameInput] = useState("");
  const [extraInput, setExtraInput] = useState("");

  const deptsQuery = useQuery({
    queryKey: ["departments"],
    queryFn: () => invoke<Department[]>("list_departments"),
  });
  const taresQuery = useQuery({
    queryKey: ["tares"],
    queryFn: () => invoke<Tare[]>("list_tares"),
  });
  const taxesQuery = useQuery({
    queryKey: ["tax_rates"],
    queryFn: () => invoke<TaxRate[]>("list_tax_rates"),
  });
  const textsQuery = useQuery({
    queryKey: ["stored_texts"],
    queryFn: () => invoke<StoredText[]>("list_stored_texts"),
  });
  const barcodesQuery = useQuery({
    queryKey: ["barcode_formats"],
    queryFn: () => invoke<BarcodeFormat[]>("list_barcode_formats"),
  });

  const syncMutation = useMutation({
    mutationFn: (scaleId: string) => invoke<number>("sync_master_tables", { scaleId }),
    onSuccess: (count) => onStatus(`Synced ${count} master table record(s) to scale.`),
    onError: (err) => onStatus(`Master table sync error: ${String(err)}`),
  });

  const handleSave = async () => {
    const numId = Number.parseInt(idInput, 10);
    if (!Number.isFinite(numId) || numId < 0) {
      onStatus("Invalid numeric ID.");
      return;
    }

    if (subTab === "departments") {
      await invoke("upsert_department", {
        dept: { id: crypto.randomUUID(), did: numId, name: nameInput.trim(), meta: makeMeta() },
      });
      await queryClient.invalidateQueries({ queryKey: ["departments"] });
    } else if (subTab === "tares") {
      await invoke("upsert_tare", {
        tare: {
          id: crypto.randomUUID(),
          cid: numId,
          name: nameInput.trim() || `Tare ${numId}`,
          weight_grams: Math.max(0, Number.parseInt(extraInput, 10) || 0),
          meta: makeMeta(),
        },
      });
      await queryClient.invalidateQueries({ queryKey: ["tares"] });
    } else if (subTab === "taxes") {
      const pct = Number.parseFloat(extraInput) || 0;
      await invoke("upsert_tax_rate", {
        tax: {
          id: crypto.randomUUID(),
          tax_id: numId,
          name: nameInput.trim() || `Tax ${numId}`,
          rate_basis_points: Math.max(0, Math.round(pct * 100)),
          meta: makeMeta(),
        },
      });
      await queryClient.invalidateQueries({ queryKey: ["tax_rates"] });
    } else if (subTab === "texts") {
      await invoke("upsert_stored_text", {
        text: {
          id: crypto.randomUUID(),
          text_id: numId,
          title: nameInput.trim() || `Message ${numId}`,
          content: extraInput.trim(),
          meta: makeMeta(),
        },
      });
      await queryClient.invalidateQueries({ queryKey: ["stored_texts"] });
    } else {
      await invoke("upsert_barcode_format", {
        fmt: {
          id: crypto.randomUUID(),
          bid: numId,
          name: nameInput.trim() || `Barcode ${numId}`,
          definition_json: JSON.stringify({
            bmc: 0,
            bt: 3,
            bpd: 1,
            subtype: 3,
            template: extraInput.trim() || "2NNAAAAWWWWWC",
          }),
          meta: makeMeta(),
        },
      });
      await queryClient.invalidateQueries({ queryKey: ["barcode_formats"] });
    }

    setNameInput("");
    setExtraInput("");
    onStatus(`Saved ${subTab} #${numId}.`);
  };

  const deptCols: ColumnDef<Department>[] = [
    { accessorKey: "did", header: "DID" },
    { accessorKey: "name", header: "Name" },
  ];
  const tareCols: ColumnDef<Tare>[] = [
    { accessorKey: "cid", header: "CID" },
    { accessorKey: "name", header: "Name" },
    { accessorKey: "weight_grams", header: "Weight (g)" },
  ];
  const taxCols: ColumnDef<TaxRate>[] = [
    { accessorKey: "tax_id", header: "Tax ID" },
    { accessorKey: "name", header: "Name" },
    {
      id: "rate",
      header: "Rate (%)",
      accessorFn: (row) => `${(row.rate_basis_points / 100).toFixed(2)}%`,
    },
  ];
  const textCols: ColumnDef<StoredText>[] = [
    { accessorKey: "text_id", header: "ID" },
    { accessorKey: "title", header: "Title" },
    { accessorKey: "content", header: "Content" },
  ];
  const barcodeCols: ColumnDef<BarcodeFormat>[] = [
    { accessorKey: "bid", header: "BID" },
    { accessorKey: "name", header: "Name" },
    { accessorKey: "definition_json", header: "Definition" },
  ];

  const extraPlaceholder =
    subTab === "tares"
      ? "Weight (g)"
      : subTab === "taxes"
        ? "Rate % (e.g. 16.00)"
        : subTab === "texts"
          ? "Legend text"
          : subTab === "barcodes"
            ? "Template (e.g. 2NNAAAAWWWWWC)"
            : "";

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap gap-1.5">
          {(["departments", "tares", "taxes", "texts", "barcodes"] as const).map((tab) => (
            <Button
              key={tab}
              size="sm"
              variant={subTab === tab ? "default" : "outline"}
              onClick={() => setSubTab(tab)}
            >
              {tab.charAt(0).toUpperCase() + tab.slice(1)}
            </Button>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <select
            value={selectedScaleId}
            onChange={(ev) => setSelectedScaleId(ev.target.value)}
            className="rounded-md border border-border bg-card px-2 py-1 text-sm"
          >
            <option value="">Select scale...</option>
            {scales.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name} ({s.ip_address})
              </option>
            ))}
          </select>
          <Button
            size="sm"
            disabled={!selectedScaleId || syncMutation.isPending}
            onClick={() => syncMutation.mutate(selectedScaleId)}
          >
            <RefreshCw className="h-3.5 w-3.5" />
            Sync to Scale
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-card p-3 text-sm">
        <input
          type="number"
          value={idInput}
          onChange={(ev) => setIdInput(ev.target.value)}
          placeholder="ID"
          className="w-20 rounded-md border border-border bg-background px-2 py-1"
        />
        <input
          type="text"
          value={nameInput}
          onChange={(ev) => setNameInput(ev.target.value)}
          placeholder="Name / Title"
          className="min-w-44 flex-1 rounded-md border border-border bg-background px-2 py-1"
        />
        {subTab !== "departments" && (
          <input
            type="text"
            value={extraInput}
            onChange={(ev) => setExtraInput(ev.target.value)}
            placeholder={extraPlaceholder}
            className="min-w-44 flex-1 rounded-md border border-border bg-background px-2 py-1"
          />
        )}
        <Button size="sm" onClick={() => void handleSave()}>
          Save
        </Button>
      </div>

      {subTab === "departments" && <DataTable columns={deptCols} data={deptsQuery.data ?? []} />}
      {subTab === "tares" && <DataTable columns={tareCols} data={taresQuery.data ?? []} />}
      {subTab === "taxes" && <DataTable columns={taxCols} data={taxesQuery.data ?? []} />}
      {subTab === "texts" && <DataTable columns={textCols} data={textsQuery.data ?? []} />}
      {subTab === "barcodes" && <DataTable columns={barcodeCols} data={barcodesQuery.data ?? []} />}
    </div>
  );
}
