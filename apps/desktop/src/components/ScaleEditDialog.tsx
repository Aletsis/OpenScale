import type { Branch, Scale } from "@openscale/client";
import { Button, Dialog } from "@openscale/ui";
import { type FormEvent, useEffect, useState } from "react";

interface ScaleEditDialogProps {
  open: boolean;
  scale: Scale | null;
  branches: Branch[];
  onClose: () => void;
  onSave: (updated: Scale) => Promise<void>;
}

const INPUT_CLASS =
  "w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm text-foreground transition-colors placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20";

export function ScaleEditDialog({ open, scale, branches, onClose, onSave }: ScaleEditDialogProps) {
  const [name, setName] = useState("");
  const [branchId, setBranchId] = useState("");
  const [ipAddress, setIpAddress] = useState("");
  const [dataPort, setDataPort] = useState("3001");
  const [servicePort, setServicePort] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    if (!scale) {
      return;
    }
    setName(scale.name);
    setBranchId(scale.branch_id ?? branches[0]?.id ?? "");
    setIpAddress(scale.ip_address);
    setDataPort(String(scale.data_port));
    setServicePort(scale.service_port ? String(scale.service_port) : "");
  }, [scale, branches]);

  const handleSubmit = async (ev: FormEvent) => {
    ev.preventDefault();
    if (!scale || !name.trim() || !ipAddress.trim()) {
      return;
    }
    setIsSaving(true);
    try {
      const parsedDataPort = Number.parseInt(dataPort, 10) || scale.data_port;
      const parsedServicePort = servicePort.trim()
        ? Number.parseInt(servicePort, 10) || null
        : null;
      await onSave({
        ...scale,
        name: name.trim(),
        branch_id: branchId || null,
        ip_address: ipAddress.trim(),
        data_port: parsedDataPort,
        service_port: parsedServicePort,
        meta: {
          ...scale.meta,
          sync_version: scale.meta.sync_version + 1,
          updated_at: new Date().toISOString(),
        },
      });
      onClose();
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Dialog
      open={open && scale !== null}
      onClose={onClose}
      title={scale ? `Editar báscula — ${scale.name}` : "Editar báscula"}
      description="Actualiza el nombre, la sucursal asignada o los parámetros de red de la báscula."
      size="md"
      closeLabel="Cerrar"
      footer={
        <>
          <Button size="sm" variant="ghost" onClick={onClose} disabled={isSaving}>
            Cancelar
          </Button>
          <Button size="sm" type="submit" form="edit-scale-form" disabled={isSaving}>
            Guardar cambios
          </Button>
        </>
      }
    >
      <form
        id="edit-scale-form"
        onSubmit={(ev) => void handleSubmit(ev)}
        className="grid grid-cols-1 gap-3.5 text-sm sm:grid-cols-2"
      >
        <label className="flex flex-col gap-1 sm:col-span-2">
          <span className="text-xs font-medium text-muted-foreground">Nombre</span>
          <input
            type="text"
            required
            value={name}
            onChange={(ev) => setName(ev.target.value)}
            className={INPUT_CLASS}
          />
        </label>

        <label className="flex flex-col gap-1 sm:col-span-2">
          <span className="text-xs font-medium text-muted-foreground">Sucursal</span>
          <select
            value={branchId}
            onChange={(ev) => setBranchId(ev.target.value)}
            className={INPUT_CLASS}
          >
            <option value="">Sin sucursal asignada</option>
            {branches.map((b) => (
              <option key={b.id} value={b.id}>
                [{b.code}] {b.name}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-muted-foreground">Dirección IP</span>
          <input
            type="text"
            required
            value={ipAddress}
            onChange={(ev) => setIpAddress(ev.target.value)}
            className={INPUT_CLASS}
          />
        </label>

        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-muted-foreground">Puerto de datos</span>
          <input
            type="number"
            required
            min={1}
            max={65535}
            value={dataPort}
            onChange={(ev) => setDataPort(ev.target.value)}
            className={INPUT_CLASS}
          />
        </label>

        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-muted-foreground">Puerto de servicio (XS)</span>
          <input
            type="number"
            min={1}
            max={65535}
            value={servicePort}
            onChange={(ev) => setServicePort(ev.target.value)}
            placeholder="28822"
            className={INPUT_CLASS}
          />
        </label>

        <div className="flex flex-col justify-center gap-0.5 rounded-md border border-border bg-muted/30 px-3 py-2 text-xs">
          <span className="text-muted-foreground">
            Serie:{" "}
            <strong className="font-mono text-foreground">{scale?.serial_number ?? "—"}</strong>
          </span>
          <span className="text-muted-foreground">
            MAC: <strong className="font-mono text-foreground">{scale?.mac_address ?? "—"}</strong>
          </span>
        </div>
      </form>
    </Dialog>
  );
}
