import { DEFAULT_BANKS_748, type Scale, type ScaleSettings748 } from "@openscale/client";
import { Badge, Button, Dialog } from "@openscale/ui";
import { invoke } from "@tauri-apps/api/core";
import { Download, Loader2, RotateCcw, Save, Send } from "lucide-react";
import { useEffect, useState } from "react";

import { ScaleBanksEditor } from "./ScaleBanksEditor.js";
import {
  DEFAULT_GLOBAL_MACHINE_PARAMS,
  type GlobalMachineParamsConfig,
} from "./ScaleSettingsView.js";

interface ScaleConfigProfileDto {
  id: string;
  store_id: string;
  scope: string;
  target_id: string;
  family: string;
  machine_params_json: string | null;
  banks_748_json: string | null;
  overrides_json: string;
  updated_at: string;
}

interface ScaleIndividualConfigDialogProps {
  open: boolean;
  scale: Scale | null;
  storeId: string;
  onClose: () => void;
  onStatus: (msg: string) => void;
}

export function ScaleIndividualConfigDialog({
  open,
  scale,
  storeId,
  onClose,
  onStatus,
}: ScaleIndividualConfigDialogProps) {
  const [tab, setTab] = useState<"machine" | "banks">("machine");
  const [profileId, setProfileId] = useState<string>(() => crypto.randomUUID());
  const [globalParams, setGlobalParams] = useState<GlobalMachineParamsConfig>(
    DEFAULT_GLOBAL_MACHINE_PARAMS,
  );
  const [globalBanks, setGlobalBanks] = useState<ScaleSettings748>(DEFAULT_BANKS_748);
  const [localParams, setLocalParams] = useState<GlobalMachineParamsConfig>(
    DEFAULT_GLOBAL_MACHINE_PARAMS,
  );
  const [localBanks, setLocalBanks] = useState<ScaleSettings748>(DEFAULT_BANKS_748);
  const [overrides, setOverrides] = useState<Set<string>>(new Set());
  const [liveRawParams, setLiveRawParams] = useState<number[] | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    if (!open || !scale) {
      return;
    }
    const load = async () => {
      try {
        const [gProf, sProf] = await Promise.all([
          invoke<ScaleConfigProfileDto | null>("get_scale_config_profile", {
            scope: "global",
            targetId: "global",
            family: scale.family,
          }),
          invoke<ScaleConfigProfileDto | null>("get_scale_config_profile", {
            scope: "scale",
            targetId: scale.id,
            family: scale.family,
          }),
        ]);

        const gParams: GlobalMachineParamsConfig = {
          ...DEFAULT_GLOBAL_MACHINE_PARAMS,
          ...(gProf?.machine_params_json
            ? (JSON.parse(gProf.machine_params_json) as Partial<GlobalMachineParamsConfig>)
            : {}),
        };
        const gBanks: ScaleSettings748 = gProf?.banks_748_json
          ? (JSON.parse(gProf.banks_748_json) as ScaleSettings748)
          : DEFAULT_BANKS_748;

        setGlobalParams(gParams);
        setGlobalBanks(gBanks);

        if (sProf) {
          setProfileId(sProf.id);
          const ov = new Set<string>(
            sProf.overrides_json ? (JSON.parse(sProf.overrides_json) as string[]) : [],
          );
          setOverrides(ov);
          const sParams = sProf.machine_params_json
            ? (JSON.parse(sProf.machine_params_json) as Partial<GlobalMachineParamsConfig>)
            : {};
          setLocalParams({ ...gParams, ...sParams });
          setLocalBanks(
            ov.has("banks_748") && sProf.banks_748_json
              ? (JSON.parse(sProf.banks_748_json) as ScaleSettings748)
              : gBanks,
          );
        } else {
          setProfileId(crypto.randomUUID());
          setOverrides(new Set());
          setLocalParams(gParams);
          setLocalBanks(gBanks);
        }
      } catch {
        // Ignore load errors
      }
    };
    void load();
  }, [open, scale]);

  const setParam = <K extends keyof GlobalMachineParamsConfig>(
    key: K,
    val: GlobalMachineParamsConfig[K],
  ) => {
    setLocalParams((prev) => ({ ...prev, [key]: val }));
    setOverrides((prev) => new Set(prev).add(key));
  };

  const resetParamToGlobal = <K extends keyof GlobalMachineParamsConfig>(key: K) => {
    setLocalParams((prev) => ({ ...prev, [key]: globalParams[key] }));
    setOverrides((prev) => {
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  };

  const handleReadLive = async () => {
    if (!scale) return;
    setIsBusy(true);
    try {
      const liveM = await invoke<GlobalMachineParamsConfig & { raw: number[] }>(
        "xs_read_machine_params",
        { host: scale.ip_address, port: scale.data_port },
      );
      setLiveRawParams(liveM.raw);
      const { raw: _raw, ...rest } = liveM;
      setLocalParams(rest);
      const liveB = await invoke<ScaleSettings748>("xs_service_read_settings", {
        host: scale.ip_address,
        port: scale.service_port,
      });
      setLocalBanks(liveB);
      onStatus(`Configuración leída en vivo desde ${scale.name}.`);
    } catch (err) {
      onStatus(`Error al leer configuración de ${scale.name}: ${String(err)}`);
    } finally {
      setIsBusy(false);
    }
  };

  const handleSaveProfile = async () => {
    if (!scale) return;
    setIsBusy(true);
    try {
      await invoke("save_scale_config_profile", {
        profile: {
          id: profileId,
          store_id: storeId,
          scope: "scale",
          target_id: scale.id,
          family: scale.family,
          machine_params_json: JSON.stringify(localParams),
          banks_748_json: JSON.stringify(localBanks),
          overrides_json: JSON.stringify(Array.from(overrides)),
          updated_at: new Date().toISOString(),
        } satisfies ScaleConfigProfileDto,
      });
      onStatus(
        `Perfil individual de ${scale.name} guardado (${overrides.size} ajuste(s) propio(s)).`,
      );
    } catch (err) {
      onStatus(`Error al guardar perfil individual: ${String(err)}`);
    } finally {
      setIsBusy(false);
    }
  };

  const handleSendToScale = async () => {
    if (!scale) return;
    setIsBusy(true);
    try {
      const raw =
        liveRawParams ??
        (
          await invoke<GlobalMachineParamsConfig & { raw: number[] }>("xs_read_machine_params", {
            host: scale.ip_address,
            port: scale.data_port,
          })
        ).raw;

      await invoke("xs_write_machine_params", {
        host: scale.ip_address,
        port: scale.data_port,
        params: { ...localParams, raw },
      });

      if (tab === "banks" || overrides.has("banks_748")) {
        await invoke("xs_service_write_settings", {
          host: scale.ip_address,
          port: scale.service_port,
          settings: localBanks,
        });
      }
      await handleSaveProfile();
      onStatus(`Configuración enviada correctamente a ${scale.name} (${scale.ip_address}).`);
    } catch (err) {
      onStatus(`Error al enviar configuración a ${scale.name}: ${String(err)}`);
    } finally {
      setIsBusy(false);
    }
  };

  const renderBadge = (key: string) =>
    overrides.has(key) ? (
      <button
        type="button"
        onClick={() =>
          key === "banks_748"
            ? (() => {
                setLocalBanks(globalBanks);
                setOverrides((prev) => {
                  const next = new Set(prev);
                  next.delete("banks_748");
                  return next;
                });
              })()
            : resetParamToGlobal(key as keyof GlobalMachineParamsConfig)
        }
        title="Restaurar al valor Global"
        className="inline-flex cursor-pointer items-center gap-1 rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] font-medium text-amber-700 hover:bg-amber-500/25 dark:text-amber-300"
      >
        Personalizado <RotateCcw className="h-2.5 w-2.5" />
      </button>
    ) : (
      <Badge variant="default" className="px-1.5 py-0 text-[10px]">
        Global
      </Badge>
    );

  return (
    <Dialog
      open={open && scale !== null}
      onClose={onClose}
      title={scale ? `Configuración Individual — ${scale.name}` : "Configuración Individual"}
      description={
        scale
          ? `${scale.ip_address} · S/N ${scale.serial_number ?? "—"} · Los campos en "Global" heredan la configuración general.`
          : ""
      }
      size="lg"
      closeLabel="Cerrar"
      footer={
        <>
          <Button
            size="sm"
            variant="outline"
            disabled={isBusy}
            onClick={() => void handleReadLive()}
          >
            <Download className="h-3.5 w-3.5" /> Leer de báscula
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={isBusy}
            onClick={() => void handleSaveProfile()}
          >
            <Save className="h-3.5 w-3.5" /> Guardar perfil
          </Button>
          <Button size="sm" disabled={isBusy} onClick={() => void handleSendToScale()}>
            {isBusy ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Send className="h-3.5 w-3.5" />
            )}
            Guardar y Enviar a Báscula
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="flex items-center justify-between gap-2">
          <div className="flex gap-1.5">
            <Button
              size="sm"
              variant={tab === "machine" ? "default" : "outline"}
              onClick={() => setTab("machine")}
            >
              Parámetros (CMD 24)
            </Button>
            <Button
              size="sm"
              variant={tab === "banks" ? "default" : "outline"}
              onClick={() => setTab("banks")}
            >
              Bancos #748 (MxBusiness)
            </Button>
          </div>
          {tab === "banks" && renderBadge("banks_748")}
        </div>

        {tab === "machine" ? (
          <div className="grid grid-cols-1 gap-3 text-xs sm:grid-cols-2 lg:grid-cols-3">
            <label className="flex flex-col gap-1 rounded border border-border p-2.5">
              <div className="flex items-center justify-between">
                <span className="font-medium">Modo de Impresión (PTM)</span>
                {renderBadge("print_mode")}
              </div>
              <select
                value={localParams.print_mode}
                onChange={(ev) => setParam("print_mode", Number(ev.target.value))}
                className="rounded border border-border bg-background px-2 py-1"
              >
                <option value={0}>0 — Bajo demanda</option>
                <option value={1}>1 — Inmediato</option>
                <option value={2}>2 — Pre-empaque</option>
              </select>
            </label>

            <label className="flex flex-col gap-1 rounded border border-border p-2.5">
              <div className="flex items-center justify-between">
                <span className="font-medium">Etiqueta por defecto (PFID)</span>
                {renderBadge("default_label_p1")}
              </div>
              <input
                type="number"
                min={0}
                max={99}
                value={localParams.default_label_p1}
                onChange={(ev) => setParam("default_label_p1", Number(ev.target.value) || 0)}
                className="rounded border border-border bg-background px-2 py-1 font-mono"
              />
            </label>

            <label className="flex flex-col gap-1 rounded border border-border p-2.5">
              <div className="flex items-center justify-between">
                <span className="font-medium">Código Barras por defecto (BID)</span>
                {renderBadge("default_barcode_id")}
              </div>
              <input
                type="number"
                min={0}
                max={32}
                value={localParams.default_barcode_id}
                onChange={(ev) => setParam("default_barcode_id", Number(ev.target.value) || 0)}
                className="rounded border border-border bg-background px-2 py-1 font-mono"
              />
            </label>

            <label className="flex flex-col gap-1 rounded border border-border p-2.5">
              <div className="flex items-center justify-between">
                <span className="font-medium">Velocidad Etiqueta (%)</span>
                {renderBadge("label_print_speed")}
              </div>
              <input
                type="number"
                min={30}
                max={100}
                value={localParams.label_print_speed}
                onChange={(ev) => setParam("label_print_speed", Number(ev.target.value) || 80)}
                className="rounded border border-border bg-background px-2 py-1 font-mono"
              />
            </label>

            <label className="flex flex-col gap-1 rounded border border-border p-2.5">
              <div className="flex items-center justify-between">
                <span className="font-medium">Autoservicio (SSM)</span>
                {renderBadge("self_service_mode")}
              </div>
              <input
                type="checkbox"
                checked={localParams.self_service_mode}
                onChange={(ev) => setParam("self_service_mode", ev.target.checked)}
                className="mt-1 h-4 w-4 accent-primary"
              />
            </label>

            <label className="flex flex-col gap-1 rounded border border-border p-2.5">
              <div className="flex items-center justify-between">
                <span className="font-medium">Barra de corte (TBRF)</span>
                {renderBadge("tear_bar_fitted")}
              </div>
              <input
                type="checkbox"
                checked={localParams.tear_bar_fitted}
                onChange={(ev) => setParam("tear_bar_fitted", ev.target.checked)}
                className="mt-1 h-4 w-4 accent-primary"
              />
            </label>
          </div>
        ) : (
          <ScaleBanksEditor
            value={localBanks}
            baseline={globalBanks}
            onChange={(next) => {
              setLocalBanks(next);
              setOverrides((prev) => new Set(prev).add("banks_748"));
            }}
          />
        )}
      </div>
    </Dialog>
  );
}
