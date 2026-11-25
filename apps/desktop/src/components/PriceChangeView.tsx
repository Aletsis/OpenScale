import type { Branch, Plu, PriceChangeItem, Scale } from "@openscale/client";
import { Badge, Button, useOpenScaleTransport } from "@openscale/ui";
import { DollarSign, Loader2, Percent, Search, Send, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";

interface PriceChangeViewProps {
  plus: Plu[];
  scales: Scale[];
  branches: Branch[];
  onUpdated: (summary: string) => void;
  onError: (msg: string) => void;
}

interface QueuedChange {
  plu: Plu;
  newPrice1: string;
  newPrice2: string;
}

export function PriceChangeView({
  plus,
  scales,
  branches,
  onUpdated,
  onError,
}: PriceChangeViewProps) {
  const transport = useOpenScaleTransport();
  const [search, setSearch] = useState("");
  const [queue, setQueue] = useState<Record<string, QueuedChange>>({});
  const [branchFilter, setBranchFilter] = useState<string>("all");
  const [selectedScaleIds, setSelectedScaleIds] = useState<Set<string>>(
    () => new Set(scales.filter((s) => s.status === "online").map((s) => s.id)),
  );
  const [sendImmediately, setSendImmediately] = useState(true);
  const [bulkPercent, setBulkPercent] = useState("5");
  const [isApplying, setIsApplying] = useState(false);

  const visibleScales = useMemo(
    () => (branchFilter === "all" ? scales : scales.filter((s) => s.branch_id === branchFilter)),
    [scales, branchFilter],
  );

  const filteredPlus = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) {
      return plus.slice(0, 80);
    }
    return plus
      .filter(
        (p) =>
          p.description.toLowerCase().includes(q) ||
          String(p.key.plu).includes(q) ||
          (p.ean ?? "").toLowerCase().includes(q),
      )
      .slice(0, 80);
  }, [plus, search]);

  const queueList = useMemo(() => Object.values(queue), [queue]);

  const addToQueue = (plu: Plu) => {
    const id = `${plu.key.did}:${plu.key.plu}`;
    setQueue((prev) => {
      if (prev[id]) {
        return prev;
      }
      return {
        ...prev,
        [id]: {
          plu,
          newPrice1: (plu.price_1 / 100).toFixed(2),
          newPrice2: (plu.price_2 / 100).toFixed(2),
        },
      };
    });
  };

  const removeFromQueue = (id: string) => {
    setQueue((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  };

  const applyPercentToQueue = () => {
    const pct = Number.parseFloat(bulkPercent);
    if (!Number.isFinite(pct) || queueList.length === 0) {
      return;
    }
    const factor = 1 + pct / 100;
    setQueue((prev) => {
      const next: Record<string, QueuedChange> = {};
      for (const [id, item] of Object.entries(prev)) {
        const current = Number.parseFloat(item.newPrice1) || item.plu.price_1 / 100;
        next[id] = {
          ...item,
          newPrice1: Math.max(0, current * factor).toFixed(2),
        };
      }
      return next;
    });
  };

  const handleApply = async () => {
    if (queueList.length === 0 || !transport.applyPriceChanges) {
      return;
    }
    const items: PriceChangeItem[] = [];
    for (const entry of queueList) {
      const p1 = Number.parseFloat(entry.newPrice1);
      const p2 = Number.parseFloat(entry.newPrice2);
      if (!Number.isFinite(p1) || p1 < 0) {
        onError(`Precio inválido para PLU ${entry.plu.key.plu}`);
        return;
      }
      items.push({
        key: entry.plu.key,
        price_1: Math.round(p1 * 100),
        price_2: Number.isFinite(p2) && p2 >= 0 ? Math.round(p2 * 100) : entry.plu.price_2,
        price_3: entry.plu.price_3,
      });
    }

    const targets = visibleScales.filter((s) => selectedScaleIds.has(s.id)).map((s) => s.id);
    setIsApplying(true);
    try {
      const report = await transport.applyPriceChanges(items, targets, sendImmediately);
      const okScales = report.scale_results.filter((r) => r.failed === 0).length;
      const totalSent = report.scale_results.reduce((acc, r) => acc + r.updated, 0);
      const totalFailed = report.scale_results.reduce((acc, r) => acc + r.failed, 0);
      setQueue({});
      onUpdated(
        sendImmediately
          ? `Cambio de precio aplicado: ${report.updated_catalog} PLU(s) actualizados en catálogo global, ${totalSent} envíos rápidos a ${okScales}/${report.scale_results.length} báscula(s)${totalFailed > 0 ? ` (${totalFailed} fallidos)` : ""}.`
          : `Cambio de precio guardado en catálogo global (${report.updated_catalog} PLU(s)).`,
      );
    } catch (err) {
      onError(String(err));
    } finally {
      setIsApplying(false);
    }
  };

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-12">
      {/* Left: Quick PLU picker */}
      <div className="space-y-3 rounded-lg border border-border bg-card p-4 lg:col-span-5">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold">Buscar PLUs (Catálogo Maestro)</h3>
          <Badge variant="default">{plus.length} PLUs</Badge>
        </div>
        <div className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <input
            type="search"
            value={search}
            onChange={(ev) => setSearch(ev.target.value)}
            placeholder="Buscar por PLU, descripción o EAN..."
            className="w-full rounded-md border border-border bg-background py-1.5 pr-3 pl-8 text-sm"
          />
        </div>
        <div className="max-h-[460px] divide-y divide-border overflow-y-auto rounded-md border border-border">
          {filteredPlus.map((plu) => {
            const id = `${plu.key.did}:${plu.key.plu}`;
            const inQueue = Boolean(queue[id]);
            return (
              <div
                key={plu.id}
                className="flex items-center justify-between gap-2 px-3 py-2 text-xs hover:bg-muted/40"
              >
                <div className="min-w-0">
                  <div className="truncate font-medium">
                    <span className="font-mono text-primary">
                      {plu.key.did}:{plu.key.plu}
                    </span>{" "}
                    {plu.description}
                  </div>
                  <div className="font-mono text-[11px] text-muted-foreground">
                    Actual: ${(plu.price_1 / 100).toFixed(2)}
                    {plu.price_2 > 0 ? ` · P2: $${(plu.price_2 / 100).toFixed(2)}` : ""}
                  </div>
                </div>
                <Button
                  size="sm"
                  variant={inQueue ? "secondary" : "outline"}
                  disabled={inQueue}
                  onClick={() => addToQueue(plu)}
                >
                  {inQueue ? "En cola" : "Agregar"}
                </Button>
              </div>
            );
          })}
        </div>
      </div>

      {/* Right: Price Change Queue + Target Scales */}
      <div className="space-y-4 rounded-lg border border-border bg-card p-4 lg:col-span-7">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <DollarSign className="h-4 w-4 text-primary" />
            <h3 className="text-sm font-semibold">Cola de Cambio de Precio ({queueList.length})</h3>
          </div>
          {queueList.length > 0 && (
            <div className="flex items-center gap-1.5 text-xs">
              <input
                type="number"
                step="0.5"
                value={bulkPercent}
                onChange={(ev) => setBulkPercent(ev.target.value)}
                className="w-16 rounded border border-border bg-background px-2 py-1 text-right font-mono"
              />
              <Button size="sm" variant="outline" onClick={applyPercentToQueue}>
                <Percent className="h-3 w-3" /> Aplicar % a cola
              </Button>
            </div>
          )}
        </div>

        {queueList.length === 0 ? (
          <div className="rounded-md border border-dashed border-border p-8 text-center text-xs text-muted-foreground">
            Selecciona uno o varios PLUs de la lista izquierda para modificar sus precios globales y
            enviarlos al instante con <code className="font-mono">CHANGE_PRICE (CMD 5)</code>.
          </div>
        ) : (
          <div className="max-h-64 divide-y divide-border overflow-y-auto rounded-md border border-border text-xs">
            {queueList.map((entry) => {
              const id = `${entry.plu.key.did}:${entry.plu.key.plu}`;
              const oldVal = entry.plu.price_1 / 100;
              const newVal = Number.parseFloat(entry.newPrice1) || 0;
              const diffPct = oldVal > 0 ? ((newVal - oldVal) / oldVal) * 100 : 0;
              return (
                <div key={id} className="grid grid-cols-12 items-center gap-2 px-3 py-2">
                  <div className="col-span-5 min-w-0">
                    <div className="truncate font-medium">
                      {entry.plu.key.plu} — {entry.plu.description}
                    </div>
                    <div className="font-mono text-[11px] text-muted-foreground">
                      Anterior: ${oldVal.toFixed(2)}{" "}
                      {diffPct !== 0 && (
                        <span className={diffPct > 0 ? "text-emerald-600" : "text-destructive"}>
                          ({diffPct > 0 ? "+" : ""}
                          {diffPct.toFixed(1)}%)
                        </span>
                      )}
                    </div>
                  </div>
                  <label className="col-span-3 flex flex-col gap-0.5">
                    <span className="text-[10px] text-muted-foreground">Nuevo P1</span>
                    <input
                      type="number"
                      step="0.01"
                      min={0}
                      value={entry.newPrice1}
                      onChange={(ev) =>
                        setQueue((prev) => ({
                          ...prev,
                          [id]: { ...entry, newPrice1: ev.target.value },
                        }))
                      }
                      className="rounded border border-border bg-background px-2 py-1 font-mono"
                    />
                  </label>
                  <label className="col-span-3 flex flex-col gap-0.5">
                    <span className="text-[10px] text-muted-foreground">Nuevo P2</span>
                    <input
                      type="number"
                      step="0.01"
                      min={0}
                      value={entry.newPrice2}
                      onChange={(ev) =>
                        setQueue((prev) => ({
                          ...prev,
                          [id]: { ...entry, newPrice2: ev.target.value },
                        }))
                      }
                      className="rounded border border-border bg-background px-2 py-1 font-mono"
                    />
                  </label>
                  <div className="col-span-1 flex justify-end">
                    <Button
                      size="icon"
                      variant="ghost"
                      onClick={() => removeFromQueue(id)}
                      title="Quitar de la cola"
                    >
                      <Trash2 className="h-3.5 w-3.5 text-destructive" />
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* Target Scales selector */}
        <div className="space-y-2.5 rounded-md border border-border bg-muted/20 p-3 text-xs">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <label className="flex cursor-pointer items-center gap-2 font-medium">
              <input
                type="checkbox"
                checked={sendImmediately}
                onChange={(ev) => setSendImmediately(ev.target.checked)}
                className="h-4 w-4 accent-primary"
              />
              <span>Enviar cambio de precio inmediatamente a las básculas seleccionadas</span>
            </label>
            <select
              value={branchFilter}
              onChange={(ev) => setBranchFilter(ev.target.value)}
              className="rounded border border-border bg-background px-2 py-1 text-xs"
            >
              <option value="all">Todas las sucursales</option>
              {branches.map((b) => (
                <option key={b.id} value={b.id}>
                  [{b.code}] {b.name}
                </option>
              ))}
            </select>
          </div>

          {sendImmediately && (
            <div className="flex flex-wrap gap-2">
              {visibleScales.map((scale) => {
                const checked = selectedScaleIds.has(scale.id);
                return (
                  <label
                    key={scale.id}
                    className={`flex cursor-pointer items-center gap-1.5 rounded border px-2 py-1 ${
                      checked
                        ? "border-primary bg-primary/10 text-foreground"
                        : "border-border bg-background text-muted-foreground"
                    }`}
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() =>
                        setSelectedScaleIds((prev) => {
                          const next = new Set(prev);
                          if (next.has(scale.id)) {
                            next.delete(scale.id);
                          } else {
                            next.add(scale.id);
                          }
                          return next;
                        })
                      }
                      className="h-3.5 w-3.5 accent-primary"
                    />
                    <span>{scale.name}</span>
                    <span className="font-mono text-[10px] opacity-75">{scale.ip_address}</span>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        <div className="flex justify-end">
          <Button
            disabled={isApplying || queueList.length === 0}
            onClick={() => void handleApply()}
          >
            {isApplying ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Send className="h-4 w-4" />
            )}
            {sendImmediately ? "Guardar y Enviar Precios" : "Guardar en Catálogo Global"}
          </Button>
        </div>
      </div>
    </div>
  );
}
