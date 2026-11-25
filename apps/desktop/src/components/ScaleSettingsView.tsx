import {
  type Branch,
  DEFAULT_BANKS_748,
  type Scale,
  type ScaleSettings748,
} from "@openscale/client";
import { Badge, Button } from "@openscale/ui";
import { invoke } from "@tauri-apps/api/core";
import {
  Download,
  FileText,
  FolderOpen,
  Globe,
  Loader2,
  RefreshCw,
  Save,
  Send,
  Sliders,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { ScaleBanksEditor } from "./ScaleBanksEditor.js";

export interface GlobalMachineParamsConfig {
  default_label_p1: number;
  default_label_p2: number;
  print_mode: number;
  default_barcode_id: number;
  label_detect: boolean;
  forced_tare: number;
  checkout_mode: number;
  pos_mode: boolean;
  self_service_mode: boolean;
  one_shot_label_mode: boolean;
  label_print_speed: number;
  receipt_print_speed: number;
  tear_bar_fitted: boolean;
}

export const DEFAULT_GLOBAL_MACHINE_PARAMS: GlobalMachineParamsConfig = {
  default_label_p1: 1,
  default_label_p2: 1,
  print_mode: 0,
  default_barcode_id: 1,
  label_detect: true,
  forced_tare: 0,
  checkout_mode: 0,
  pos_mode: false,
  self_service_mode: false,
  one_shot_label_mode: false,
  label_print_speed: 80,
  receipt_print_speed: 80,
  tear_bar_fitted: false,
};

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

interface LogFileEntryDto {
  name: string;
  path: string;
  size_bytes: number;
  modified_at: string | null;
  is_current: boolean;
}

interface LogInfoDto {
  log_dir: string;
  current_log_file: string;
  current_size_bytes: number;
  files: LogFileEntryDto[];
}

interface LogReadResultDto {
  file_name: string;
  file_path: string;
  total_lines: number;
  returned_lines: number;
  content: string;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

interface ScaleSettingsViewProps {
  scales: Scale[];
  branches: Branch[];
  storeId: string;
  onStatus: (msg: string) => void;
}

export function ScaleSettingsView({ scales, branches, storeId, onStatus }: ScaleSettingsViewProps) {
  const [subTab, setSubTab] = useState<"machine" | "banks" | "app">("machine");
  const [profileId, setProfileId] = useState<string>(() => crypto.randomUUID());
  const [machineParams, setMachineParams] = useState<GlobalMachineParamsConfig>(
    DEFAULT_GLOBAL_MACHINE_PARAMS,
  );
  const [banks748, setBanks748] = useState<ScaleSettings748>(DEFAULT_BANKS_748);
  const [savedBanks748, setSavedBanks748] = useState<ScaleSettings748>(DEFAULT_BANKS_748);

  // App general settings
  const [healthIntervalSecs, setHealthIntervalSecs] = useState(15);
  const [currencySymbol, setCurrencySymbol] = useState("$");
  const [blockSyncOnIdentityConflict, setBlockSyncOnIdentityConflict] = useState(true);

  // Application log viewer state
  const [logInfo, setLogInfo] = useState<LogInfoDto | null>(null);
  const [selectedLogFile, setSelectedLogFile] = useState<string>("");
  const [logLevelFilter, setLogLevelFilter] = useState<"ALL" | "ERROR" | "WARN">("ALL");
  const [logResult, setLogResult] = useState<LogReadResultDto | null>(null);
  const [isLoadingLogs, setIsLoadingLogs] = useState(false);

  const loadLogs = useCallback(
    async (fileNameOverride?: string, filterOverride?: "ALL" | "ERROR" | "WARN") => {
      setIsLoadingLogs(true);
      try {
        const info = await invoke<LogInfoDto>("get_log_info");
        setLogInfo(info);
        const targetFile =
          fileNameOverride !== undefined
            ? fileNameOverride
            : selectedLogFile || info.files[0]?.name || "";
        if (!selectedLogFile && targetFile) {
          setSelectedLogFile(targetFile);
        }
        const activeFilter = filterOverride ?? logLevelFilter;
        const result = await invoke<LogReadResultDto>("read_log_entries", {
          fileName: targetFile || null,
          maxLines: 500,
          levelFilter: activeFilter === "ALL" ? null : activeFilter,
        });
        setLogResult(result);
      } catch (err) {
        onStatus(`Error leyendo logs: ${String(err)}`);
      } finally {
        setIsLoadingLogs(false);
      }
    },
    [logLevelFilter, onStatus, selectedLogFile],
  );

  useEffect(() => {
    if (subTab === "app") {
      void loadLogs();
    }
  }, [subTab, loadLogs]);

  const handleOpenLogsDir = async () => {
    try {
      await invoke("open_logs_dir");
    } catch (err) {
      onStatus(`No se pudo abrir la carpeta de logs: ${String(err)}`);
    }
  };

  const handleDownloadLog = async () => {
    try {
      const targetFile = selectedLogFile || logInfo?.files[0]?.name || null;
      const full = await invoke<LogReadResultDto>("read_log_entries", {
        fileName: targetFile,
        maxLines: 10000,
        levelFilter: null,
      });
      const blob = new Blob([full.content], { type: "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = full.file_name || "openscale.log";
      anchor.click();
      URL.revokeObjectURL(url);
      onStatus(`Log exportado: ${full.file_name}`);
    } catch (err) {
      onStatus(`Error exportando log: ${String(err)}`);
    }
  };

  // Apply to scales state
  const [branchFilter, setBranchFilter] = useState<string>("all");
  const [selectedScaleIds, setSelectedScaleIds] = useState<Set<string>>(new Set());
  const [isSaving, setIsSaving] = useState(false);
  const [isApplying, setIsApplying] = useState(false);

  const xsScales = useMemo(() => {
    const list = scales.filter((s) => s.family === "avery_berkel_xs");
    return branchFilter === "all" ? list : list.filter((s) => s.branch_id === branchFilter);
  }, [scales, branchFilter]);

  useEffect(() => {
    const load = async () => {
      try {
        const profile = await invoke<ScaleConfigProfileDto | null>("get_scale_config_profile", {
          scope: "global",
          targetId: "global",
          family: "avery_berkel_xs",
        });
        if (profile) {
          setProfileId(profile.id);
          if (profile.machine_params_json) {
            setMachineParams({
              ...DEFAULT_GLOBAL_MACHINE_PARAMS,
              ...(JSON.parse(profile.machine_params_json) as Partial<GlobalMachineParamsConfig>),
            });
          }
          if (profile.banks_748_json) {
            const parsed = JSON.parse(profile.banks_748_json) as ScaleSettings748;
            setBanks748(parsed);
            setSavedBanks748(parsed);
          }
        }
        const generalRaw = await invoke<string | null>("get_app_setting", { key: "general" });
        if (generalRaw) {
          const parsed = JSON.parse(generalRaw) as {
            health_interval_secs?: number;
            currency_symbol?: string;
            block_sync_on_identity_conflict?: boolean;
          };
          if (parsed.health_interval_secs) setHealthIntervalSecs(parsed.health_interval_secs);
          if (parsed.currency_symbol) setCurrencySymbol(parsed.currency_symbol);
          if (typeof parsed.block_sync_on_identity_conflict === "boolean") {
            setBlockSyncOnIdentityConflict(parsed.block_sync_on_identity_conflict);
          }
        }
      } catch {
        // Use defaults
      }
    };
    void load();
  }, []);

  const handleSaveGlobal = async () => {
    setIsSaving(true);
    try {
      await invoke("save_scale_config_profile", {
        profile: {
          id: profileId,
          store_id: storeId,
          scope: "global",
          target_id: "global",
          family: "avery_berkel_xs",
          machine_params_json: JSON.stringify(machineParams),
          banks_748_json: JSON.stringify(banks748),
          overrides_json: "[]",
          updated_at: new Date().toISOString(),
        } satisfies ScaleConfigProfileDto,
      });
      await invoke("set_app_setting", {
        key: "general",
        valueJson: JSON.stringify({
          health_interval_secs: healthIntervalSecs,
          currency_symbol: currencySymbol,
          block_sync_on_identity_conflict: blockSyncOnIdentityConflict,
        }),
      });
      setSavedBanks748(banks748);
      onStatus("Configuración global guardada correctamente.");
    } catch (err) {
      onStatus(`Error al guardar configuración global: ${String(err)}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleApplyGlobalToSelected = async (applyBanks: boolean) => {
    const targets = xsScales.filter((s) => selectedScaleIds.has(s.id));
    if (targets.length === 0) {
      onStatus("Selecciona al menos una báscula XS para aplicar la configuración.");
      return;
    }
    setIsApplying(true);
    let ok = 0;
    let fail = 0;
    try {
      for (const scale of targets) {
        try {
          // Load individual overrides if present so we respect per-scale customizations
          const indiv = await invoke<ScaleConfigProfileDto | null>("get_scale_config_profile", {
            scope: "scale",
            targetId: scale.id,
            family: scale.family,
          });
          const overrideKeys = new Set<string>(
            indiv?.overrides_json ? (JSON.parse(indiv.overrides_json) as string[]) : [],
          );
          const indivParams = indiv?.machine_params_json
            ? (JSON.parse(indiv.machine_params_json) as Partial<GlobalMachineParamsConfig>)
            : {};

          // Read live 80-byte MACHINE_PARAMETERS first to preserve unknown bytes safely
          const liveParams = await invoke<GlobalMachineParamsConfig & { raw: number[] }>(
            "xs_read_machine_params",
            { host: scale.ip_address, port: scale.data_port },
          );

          const merged = { ...liveParams };
          for (const key of Object.keys(DEFAULT_GLOBAL_MACHINE_PARAMS) as Array<
            keyof GlobalMachineParamsConfig
          >) {
            const val =
              overrideKeys.has(key) && indivParams[key] !== undefined
                ? indivParams[key]
                : machineParams[key];
            (merged as Record<string, unknown>)[key] = val;
          }

          await invoke("xs_write_machine_params", {
            host: scale.ip_address,
            port: scale.data_port,
            params: merged,
          });

          if (applyBanks && !overrideKeys.has("banks_748")) {
            await invoke("xs_service_write_settings", {
              host: scale.ip_address,
              port: scale.service_port,
              settings: banks748,
            });
          }
          ok += 1;
        } catch {
          fail += 1;
        }
      }
      onStatus(
        `Configuración global aplicada a ${ok}/${targets.length} báscula(s) XS${fail > 0 ? ` (${fail} fallidas)` : ""}.`,
      );
    } finally {
      setIsApplying(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Sub-navigation + Save */}
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-card p-3">
        <div className="flex flex-wrap gap-1.5">
          <Button
            size="sm"
            variant={subTab === "machine" ? "default" : "outline"}
            onClick={() => setSubTab("machine")}
          >
            <Sliders className="h-3.5 w-3.5" /> Parámetros Globales (CMD 24)
          </Button>
          <Button
            size="sm"
            variant={subTab === "banks" ? "default" : "outline"}
            onClick={() => setSubTab("banks")}
          >
            Bancos #748 (Estilo MxBusiness)
          </Button>
          <Button
            size="sm"
            variant={subTab === "app" ? "default" : "outline"}
            onClick={() => setSubTab("app")}
          >
            <Globe className="h-3.5 w-3.5" /> Aplicación
          </Button>
        </div>
        <Button size="sm" disabled={isSaving} onClick={() => void handleSaveGlobal()}>
          {isSaving ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Save className="h-3.5 w-3.5" />
          )}
          Guardar Configuración Global
        </Button>
      </div>

      {subTab === "machine" && (
        <div className="space-y-4 rounded-md border border-border bg-card p-4 text-sm">
          <div>
            <h3 className="font-semibold">Parámetros Globales de Básculas (MACHINE_PARAMETERS)</h3>
            <p className="text-xs text-muted-foreground">
              Todas las básculas heredan estos valores salvo que tengan parámetros personalizados en
              su configuración individual.
            </p>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Modo de Impresión (PTM)</span>
              <select
                value={machineParams.print_mode}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, print_mode: Number(ev.target.value) })
                }
                className="rounded border border-border bg-background px-2 py-1.5"
              >
                <option value={0}>0 — Bajo demanda / Mostrador</option>
                <option value={1}>1 — Inmediato</option>
                <option value={2}>2 — Pre-empaque</option>
              </select>
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Etiqueta por defecto P1 (PFID)</span>
              <input
                type="number"
                min={0}
                max={99}
                value={machineParams.default_label_p1}
                onChange={(ev) =>
                  setMachineParams({
                    ...machineParams,
                    default_label_p1: Number(ev.target.value) || 0,
                  })
                }
                className="rounded border border-border bg-background px-2 py-1.5 font-mono"
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Código Barras por defecto (BID)</span>
              <input
                type="number"
                min={0}
                max={32}
                value={machineParams.default_barcode_id}
                onChange={(ev) =>
                  setMachineParams({
                    ...machineParams,
                    default_barcode_id: Number(ev.target.value) || 0,
                  })
                }
                className="rounded border border-border bg-background px-2 py-1.5 font-mono"
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Tara Forzada (FT)</span>
              <select
                value={machineParams.forced_tare}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, forced_tare: Number(ev.target.value) })
                }
                className="rounded border border-border bg-background px-2 py-1.5"
              >
                <option value={0}>0 — Sin restricción</option>
                <option value={1}>1 — Siempre requerida</option>
                <option value={2}>2 — Manual no sobrescribible</option>
              </select>
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Velocidad Etiqueta (%)</span>
              <input
                type="number"
                min={30}
                max={100}
                value={machineParams.label_print_speed}
                onChange={(ev) =>
                  setMachineParams({
                    ...machineParams,
                    label_print_speed: Number(ev.target.value) || 80,
                  })
                }
                className="rounded border border-border bg-background px-2 py-1.5 font-mono"
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Velocidad Recibo (%)</span>
              <input
                type="number"
                min={30}
                max={100}
                value={machineParams.receipt_print_speed}
                onChange={(ev) =>
                  setMachineParams({
                    ...machineParams,
                    receipt_print_speed: Number(ev.target.value) || 80,
                  })
                }
                className="rounded border border-border bg-background px-2 py-1.5 font-mono"
              />
            </label>
            <label className="flex items-center gap-2 pt-5">
              <input
                type="checkbox"
                checked={machineParams.label_detect}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, label_detect: ev.target.checked })
                }
                className="h-4 w-4 accent-primary"
              />
              <span>Sensor de etiqueta (LDT)</span>
            </label>
            <label className="flex items-center gap-2 pt-5">
              <input
                type="checkbox"
                checked={machineParams.self_service_mode}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, self_service_mode: ev.target.checked })
                }
                className="h-4 w-4 accent-primary"
              />
              <span>Autoservicio (SSM)</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={machineParams.pos_mode}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, pos_mode: ev.target.checked })
                }
                className="h-4 w-4 accent-primary"
              />
              <span>Modo PoS (POSM)</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={machineParams.one_shot_label_mode}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, one_shot_label_mode: ev.target.checked })
                }
                className="h-4 w-4 accent-primary"
              />
              <span>Un solo disparo (OSLM)</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={machineParams.tear_bar_fitted}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, tear_bar_fitted: ev.target.checked })
                }
                className="h-4 w-4 accent-primary"
              />
              <span>Barra de corte (TBRF)</span>
            </label>
          </div>
        </div>
      )}

      {subTab === "banks" && (
        <div className="rounded-md border border-border bg-card p-4">
          <ScaleBanksEditor value={banks748} baseline={savedBanks748} onChange={setBanks748} />
        </div>
      )}

      {subTab === "app" && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 gap-4 rounded-md border border-border bg-card p-4 text-sm sm:grid-cols-3">
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">
                Intervalo monitoreo de red (seg)
              </span>
              <input
                type="number"
                min={5}
                max={300}
                value={healthIntervalSecs}
                onChange={(ev) => setHealthIntervalSecs(Math.max(5, Number(ev.target.value) || 15))}
                className="rounded border border-border bg-background px-2.5 py-1.5 font-mono"
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">Símbolo de moneda</span>
              <input
                type="text"
                maxLength={4}
                value={currencySymbol}
                onChange={(ev) => setCurrencySymbol(ev.target.value)}
                className="rounded border border-border bg-background px-2.5 py-1.5 font-mono"
              />
            </label>
            <label className="flex items-center gap-2 pt-5">
              <input
                type="checkbox"
                checked={blockSyncOnIdentityConflict}
                onChange={(ev) => setBlockSyncOnIdentityConflict(ev.target.checked)}
                className="h-4 w-4 accent-primary"
              />
              <span className="text-xs">Bloquear sync automático si hay conflicto de IP/Serie</span>
            </label>
          </div>

          <div className="space-y-3 rounded-md border border-border bg-card p-4 text-xs">
            <div className="flex flex-wrap items-start justify-between gap-2">
              <div className="space-y-1">
                <h3 className="flex items-center gap-1.5 text-sm font-semibold">
                  <FileText className="h-4 w-4 text-primary" />
                  Archivo de Logs y Diagnóstico
                </h3>
                <p className="text-muted-foreground">
                  Los errores detallados de backend, drivers de básculas, base de datos, panics e
                  interfaz se guardan automáticamente con rotación diaria.
                </p>
                {logInfo && (
                  <p className="font-mono text-[11px] text-muted-foreground select-all">
                    Archivo activo: {logInfo.current_log_file} (
                    {formatBytes(logInfo.current_size_bytes)})
                  </p>
                )}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Button size="sm" variant="outline" onClick={() => void handleOpenLogsDir()}>
                  <FolderOpen className="h-3.5 w-3.5" />
                  Abrir carpeta
                </Button>
                <Button size="sm" variant="outline" onClick={() => void handleDownloadLog()}>
                  <Download className="h-3.5 w-3.5" />
                  Descargar log
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={isLoadingLogs}
                  onClick={() => void loadLogs()}
                >
                  {isLoadingLogs ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <RefreshCw className="h-3.5 w-3.5" />
                  )}
                  Actualizar
                </Button>
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              {logInfo && logInfo.files.length > 0 && (
                <select
                  value={selectedLogFile || logInfo.files[0]?.name || ""}
                  onChange={(ev) => {
                    const next = ev.target.value;
                    setSelectedLogFile(next);
                    void loadLogs(next, logLevelFilter);
                  }}
                  className="rounded border border-border bg-background px-2 py-1 font-mono text-xs"
                >
                  {logInfo.files.map((file) => (
                    <option key={file.name} value={file.name}>
                      {file.name} ({formatBytes(file.size_bytes)})
                      {file.is_current ? " — actual" : ""}
                    </option>
                  ))}
                </select>
              )}
              <select
                value={logLevelFilter}
                onChange={(ev) => {
                  const next = ev.target.value as "ALL" | "ERROR" | "WARN";
                  setLogLevelFilter(next);
                  void loadLogs(selectedLogFile, next);
                }}
                className="rounded border border-border bg-background px-2 py-1 text-xs"
              >
                <option value="ALL">Todos los niveles</option>
                <option value="ERROR">Solo ERROR / PANIC</option>
                <option value="WARN">WARN + ERROR</option>
              </select>
              {logResult && (
                <span className="text-muted-foreground">
                  Mostrando {logResult.returned_lines} de {logResult.total_lines} líneas
                </span>
              )}
            </div>

            <div className="max-h-80 overflow-auto rounded border border-border bg-background p-3 font-mono text-[11px] leading-relaxed">
              {!logResult || !logResult.content ? (
                <span className="text-muted-foreground">
                  No hay entradas registradas para el filtro seleccionado.
                </span>
              ) : (
                logResult.content.split("\n").map((line, idx) => {
                  const isError = line.includes(" [ERROR] ") || line.includes(" [PANIC] ");
                  const isWarn = line.includes(" [WARN] ");
                  return (
                    <div
                      key={`${idx}-${line.slice(0, 24)}`}
                      className={
                        isError
                          ? "whitespace-pre-wrap break-all text-destructive"
                          : isWarn
                            ? "whitespace-pre-wrap break-all text-amber-600 dark:text-amber-400"
                            : "whitespace-pre-wrap break-all text-foreground/90"
                      }
                    >
                      {line}
                    </div>
                  );
                })
              )}
            </div>
          </div>
        </div>
      )}

      {/* Push Global Config to Selected Scales */}
      {subTab !== "app" && (
        <div className="space-y-3 rounded-md border border-border bg-card p-4 text-xs">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h3 className="text-sm font-semibold">Aplicar Configuración Global a Básculas</h3>
              <p className="text-muted-foreground">
                Respeta automáticamente los parámetros personalizados en la configuración individual
                de cada báscula.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <select
                value={branchFilter}
                onChange={(ev) => setBranchFilter(ev.target.value)}
                className="rounded border border-border bg-background px-2 py-1"
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
                onClick={() => setSelectedScaleIds(new Set(xsScales.map((s) => s.id)))}
              >
                Seleccionar todas
              </Button>
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            {xsScales.length === 0 ? (
              <span className="text-muted-foreground">No hay básculas XS en esta sucursal.</span>
            ) : (
              xsScales.map((s) => {
                const checked = selectedScaleIds.has(s.id);
                return (
                  <label
                    key={s.id}
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
                          if (next.has(s.id)) next.delete(s.id);
                          else next.add(s.id);
                          return next;
                        })
                      }
                      className="h-3.5 w-3.5 accent-primary"
                    />
                    <span>{s.name}</span>
                    <Badge variant={s.status === "online" ? "success" : "default"}>
                      {s.ip_address}
                    </Badge>
                  </label>
                );
              })
            )}
          </div>

          <div className="flex flex-wrap justify-end gap-2">
            <Button
              size="sm"
              variant="secondary"
              disabled={isApplying || selectedScaleIds.size === 0}
              onClick={() => void handleApplyGlobalToSelected(false)}
            >
              {isApplying ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Send className="h-3.5 w-3.5" />
              )}
              Enviar Parámetros (CMD 24)
            </Button>
            <Button
              size="sm"
              disabled={isApplying || selectedScaleIds.size === 0}
              onClick={() => void handleApplyGlobalToSelected(true)}
            >
              {isApplying ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Send className="h-3.5 w-3.5" />
              )}
              Enviar Parámetros + Bancos #748
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
