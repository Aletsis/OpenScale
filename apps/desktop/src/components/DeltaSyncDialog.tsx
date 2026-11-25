import type { Branch, DeltaSyncPreview, Scale, SyncReport } from "@openscale/client";
import { Badge, Button, Dialog, useOpenScaleTransport } from "@openscale/ui";
import { CheckCircle2, Loader2, RefreshCw, Send, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

interface DeltaSyncDialogProps {
  open: boolean;
  scales: Scale[];
  branches: Branch[];
  initialScaleId?: string | null;
  onClose: () => void;
  onCompleted: (reports: Array<{ scale: Scale; report: SyncReport }>) => void;
}

export function DeltaSyncDialog({
  open,
  scales,
  branches,
  initialScaleId,
  onClose,
  onCompleted,
}: DeltaSyncDialogProps) {
  const transport = useOpenScaleTransport();
  const [branchFilter, setBranchFilter] = useState<string>("all");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [includeDeletes, setIncludeDeletes] = useState(false);
  const [previews, setPreviews] = useState<Record<string, DeltaSyncPreview>>({});
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [isSending, setIsSending] = useState(false);

  const visibleScales = useMemo(() => {
    if (branchFilter === "all") {
      return scales;
    }
    return scales.filter((s) => s.branch_id === branchFilter);
  }, [scales, branchFilter]);

  useEffect(() => {
    if (!open) {
      return;
    }
    if (initialScaleId) {
      setSelectedIds(new Set([initialScaleId]));
    } else {
      setSelectedIds(new Set(scales.filter((s) => s.status === "online").map((s) => s.id)));
    }
  }, [open, initialScaleId, scales]);

  const loadPreviews = useCallback(async () => {
    const previewDeltaSync = transport.previewDeltaSync;
    if (!previewDeltaSync || visibleScales.length === 0) {
      return;
    }
    setIsLoadingPreview(true);
    try {
      const next: Record<string, DeltaSyncPreview> = {};
      await Promise.all(
        visibleScales.map(async (scale) => {
          try {
            next[scale.id] = await previewDeltaSync(scale.id);
          } catch {
            // Skip failed preview
          }
        }),
      );
      setPreviews(next);
    } finally {
      setIsLoadingPreview(false);
    }
  }, [transport, visibleScales]);

  useEffect(() => {
    if (open) {
      void loadPreviews();
    }
  }, [open, loadPreviews]);

  const toggleScale = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const selectedList = useMemo(
    () => visibleScales.filter((s) => selectedIds.has(s.id)),
    [visibleScales, selectedIds],
  );

  const totalPendingToSend = useMemo(() => {
    return selectedList.reduce((acc, s) => {
      const p = previews[s.id];
      if (!p) {
        return acc;
      }
      return (
        acc +
        p.new_plus.length +
        p.modified_plus.length +
        (includeDeletes ? p.deleted_keys.length : 0)
      );
    }, 0);
  }, [selectedList, previews, includeDeletes]);

  const handleSend = async () => {
    if (selectedList.length === 0) {
      return;
    }
    setIsSending(true);
    try {
      const reports: Array<{ scale: Scale; report: SyncReport }> = [];
      for (const scale of selectedList) {
        const report = transport.syncScaleWithOptions
          ? await transport.syncScaleWithOptions(scale.id, "delta", includeDeletes)
          : await transport.syncScale(scale.id, "delta");
        reports.push({ scale, report });
      }
      onCompleted(reports);
      onClose();
    } finally {
      setIsSending(false);
    }
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Enviar solo PLUs nuevos o con cambios"
      description="Compara el catálogo efectivo de cada sucursal contra el último estado enviado a cada báscula y transmite únicamente los cambios."
      size="lg"
      closeLabel="Cerrar"
      footer={
        <>
          <Button size="sm" variant="ghost" onClick={onClose} disabled={isSending}>
            Cancelar
          </Button>
          <Button
            size="sm"
            disabled={isSending || selectedList.length === 0}
            onClick={() => void handleSend()}
          >
            {isSending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Send className="h-3.5 w-3.5" />
            )}
            {isSending
              ? "Enviando cambios..."
              : `Enviar cambios (${totalPendingToSend} op. a ${selectedList.length} báscula(s))`}
          </Button>
        </>
      }
    >
      <div className="space-y-3 text-sm">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={branchFilter}
              onChange={(ev) => setBranchFilter(ev.target.value)}
              className="rounded-md border border-border bg-background px-2.5 py-1 text-xs"
            >
              <option value="all">Todas las sucursales</option>
              {branches.map((b) => (
                <option key={b.id} value={b.id}>
                  [{b.code}] {b.name}
                </option>
              ))}
            </select>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setSelectedIds(new Set(visibleScales.map((s) => s.id)))}
            >
              Seleccionar todas
            </Button>
            <Button size="sm" variant="ghost" onClick={() => void loadPreviews()}>
              <RefreshCw
                className={isLoadingPreview ? "h-3.5 w-3.5 animate-spin" : "h-3.5 w-3.5"}
              />
              Recalcular
            </Button>
          </div>

          <label className="flex cursor-pointer items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              checked={includeDeletes}
              onChange={(ev) => setIncludeDeletes(ev.target.checked)}
              className="h-3.5 w-3.5 accent-primary"
            />
            <Trash2 className="h-3.5 w-3.5 text-destructive" />
            <span>Eliminar de la báscula los PLUs dados de baja</span>
          </label>
        </div>

        <div className="max-h-80 divide-y divide-border overflow-y-auto rounded-md border border-border">
          {visibleScales.length === 0 ? (
            <div className="p-4 text-center text-xs text-muted-foreground">
              No hay básculas en la sucursal seleccionada.
            </div>
          ) : (
            visibleScales.map((scale) => {
              const preview = previews[scale.id];
              const checked = selectedIds.has(scale.id);
              const pending =
                (preview?.new_plus.length ?? 0) + (preview?.modified_plus.length ?? 0);
              return (
                <label
                  key={scale.id}
                  className="flex cursor-pointer flex-wrap items-center justify-between gap-2 px-3 py-2.5 hover:bg-muted/40"
                >
                  <div className="flex items-center gap-2.5">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggleScale(scale.id)}
                      className="h-4 w-4 accent-primary"
                    />
                    <div>
                      <div className="font-medium">
                        {scale.name}{" "}
                        <span className="font-mono text-xs text-muted-foreground">
                          ({scale.ip_address})
                        </span>
                      </div>
                      {preview && preview.new_plus.length + preview.modified_plus.length > 0 && (
                        <p className="text-[11px] text-muted-foreground">
                          Ejemplos:{" "}
                          {[...preview.new_plus, ...preview.modified_plus]
                            .slice(0, 3)
                            .map((p) => `${p.key.plu} ${p.description}`)
                            .join(" · ")}
                        </p>
                      )}
                    </div>
                  </div>
                  <div className="flex flex-wrap items-center gap-1.5 text-xs">
                    {isLoadingPreview && !preview ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
                    ) : preview ? (
                      <>
                        {pending === 0 && preview.deleted_keys.length === 0 ? (
                          <Badge variant="success" className="gap-1">
                            <CheckCircle2 className="h-3 w-3" /> Al día ({preview.unchanged_count})
                          </Badge>
                        ) : (
                          <>
                            <Badge variant="default">+{preview.new_plus.length} nuevos</Badge>
                            <Badge variant="warning">
                              ~{preview.modified_plus.length} modificados
                            </Badge>
                            {preview.deleted_keys.length > 0 && (
                              <Badge variant="danger">-{preview.deleted_keys.length} bajas</Badge>
                            )}
                            <span className="text-[11px] text-muted-foreground">
                              ({preview.unchanged_count} sin cambios)
                            </span>
                          </>
                        )}
                      </>
                    ) : null}
                  </div>
                </label>
              );
            })
          )}
        </div>
      </div>
    </Dialog>
  );
}
