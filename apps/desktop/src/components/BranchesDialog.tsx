import type { Branch, BranchCatalogMode } from "@openscale/client";
import { Badge, Button, Dialog } from "@openscale/ui";
import { Building2, Pencil, Plus, Trash2 } from "lucide-react";
import { type FormEvent, useState } from "react";

interface BranchesDialogProps {
  open: boolean;
  branches: Branch[];
  storeId: string;
  onClose: () => void;
  onSave: (branch: Branch) => Promise<void>;
  onDelete: (branch: Branch) => Promise<void>;
}

const INPUT_CLASS =
  "w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm text-foreground transition-colors placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20";

export function BranchesDialog({
  open,
  branches,
  storeId,
  onClose,
  onSave,
  onDelete,
}: BranchesDialogProps) {
  const [editing, setEditing] = useState<Branch | null>(null);
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [catalogMode, setCatalogMode] = useState<BranchCatalogMode>("all");
  const [isSaving, setIsSaving] = useState(false);

  const resetForm = () => {
    setEditing(null);
    setCode("");
    setName("");
    setAddress("");
    setCatalogMode("all");
  };

  const startEdit = (branch: Branch) => {
    setEditing(branch);
    setCode(branch.code);
    setName(branch.name);
    setAddress(branch.address ?? "");
    setCatalogMode(branch.catalog_mode);
  };

  const handleSubmit = async (ev: FormEvent) => {
    ev.preventDefault();
    const trimmedCode = code.trim().toUpperCase();
    const trimmedName = name.trim();
    if (!trimmedCode || !trimmedName) {
      return;
    }

    setIsSaving(true);
    try {
      const now = new Date().toISOString();
      const payload: Branch = editing
        ? {
            ...editing,
            code: trimmedCode,
            name: trimmedName,
            address: address.trim() || null,
            catalog_mode: catalogMode,
            meta: {
              ...editing.meta,
              sync_version: editing.meta.sync_version + 1,
              updated_at: now,
            },
          }
        : {
            id: crypto.randomUUID(),
            code: trimmedCode,
            name: trimmedName,
            address: address.trim() || null,
            notes: null,
            catalog_mode: catalogMode,
            meta: {
              store_id: storeId,
              sync_version: 1,
              updated_at: now,
              deleted_at: null,
            },
          };
      await onSave(payload);
      resetForm();
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Sucursales de Básculas"
      description="Administra las sucursales. Todas comparten los mismos precios globales, pero cada sucursal puede manejar un catálogo distinto de PLUs."
      size="lg"
      closeLabel="Cerrar"
    >
      <div className="space-y-4">
        <form
          onSubmit={(ev) => void handleSubmit(ev)}
          className="grid grid-cols-1 gap-3 rounded-md border border-border bg-muted/30 p-3 text-sm sm:grid-cols-12"
        >
          <label className="flex flex-col gap-1 sm:col-span-3">
            <span className="text-xs font-medium text-muted-foreground">Código</span>
            <input
              type="text"
              required
              maxLength={16}
              value={code}
              onChange={(ev) => setCode(ev.target.value)}
              placeholder="SUC01"
              className={INPUT_CLASS}
            />
          </label>
          <label className="flex flex-col gap-1 sm:col-span-4">
            <span className="text-xs font-medium text-muted-foreground">Nombre</span>
            <input
              type="text"
              required
              value={name}
              onChange={(ev) => setName(ev.target.value)}
              placeholder="Sucursal Centro"
              className={INPUT_CLASS}
            />
          </label>
          <label className="flex flex-col gap-1 sm:col-span-5">
            <span className="text-xs font-medium text-muted-foreground">Catálogo de PLUs</span>
            <select
              value={catalogMode}
              onChange={(ev) => setCatalogMode(ev.target.value as BranchCatalogMode)}
              className={INPUT_CLASS}
            >
              <option value="all">Todos los PLUs del Catálogo Maestro</option>
              <option value="selected">Catálogo personalizado (seleccionar PLUs)</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 sm:col-span-8">
            <span className="text-xs font-medium text-muted-foreground">Dirección (opcional)</span>
            <input
              type="text"
              value={address}
              onChange={(ev) => setAddress(ev.target.value)}
              placeholder="Calle, colonia o referencia"
              className={INPUT_CLASS}
            />
          </label>
          <div className="flex items-end justify-end gap-2 sm:col-span-4">
            {editing && (
              <Button type="button" size="sm" variant="ghost" onClick={resetForm}>
                Cancelar
              </Button>
            )}
            <Button type="submit" size="sm" disabled={isSaving}>
              <Plus className="h-3.5 w-3.5" />
              {editing ? "Actualizar" : "Agregar"}
            </Button>
          </div>
        </form>

        <div className="divide-y divide-border rounded-md border border-border">
          {branches.length === 0 ? (
            <div className="p-4 text-center text-xs text-muted-foreground">
              No hay sucursales registradas.
            </div>
          ) : (
            branches.map((branch) => (
              <div
                key={branch.id}
                className="flex flex-wrap items-center justify-between gap-2 px-3 py-2.5 text-sm"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <Building2 className="h-4 w-4 text-primary" />
                    <span className="font-mono text-xs font-semibold text-primary">
                      {branch.code}
                    </span>
                    <span className="font-medium">{branch.name}</span>
                    <Badge variant={branch.catalog_mode === "selected" ? "warning" : "default"}>
                      {branch.catalog_mode === "selected" ? "Catálogo propio" : "Todos los PLUs"}
                    </Badge>
                  </div>
                  {branch.address && (
                    <p className="mt-0.5 truncate text-xs text-muted-foreground">
                      {branch.address}
                    </p>
                  )}
                </div>
                <div className="flex items-center gap-1">
                  <Button
                    size="icon"
                    variant="ghost"
                    onClick={() => startEdit(branch)}
                    title="Editar sucursal"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    disabled={branches.length <= 1}
                    onClick={() => void onDelete(branch)}
                    title="Eliminar sucursal"
                  >
                    <Trash2 className="h-3.5 w-3.5 text-destructive" />
                  </Button>
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </Dialog>
  );
}
