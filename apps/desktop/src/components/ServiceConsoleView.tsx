import { type Scale, decode1BppBitmapToRgba } from "@openscale/client";
import { Badge, Button } from "@openscale/ui";
import { invoke } from "@tauri-apps/api/core";
import { Monitor, Power, Printer, RefreshCw, Scale as ScaleIcon, Wrench } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface ServiceWeightReading {
  raw: string;
  weight_display: string;
  status: string;
}

interface ScaleSettings748 {
  version: string;
  bank1: string;
  bank2: string;
  bank3: string;
  bank4: string;
}

interface LcdFrameDto {
  width: number;
  height: number;
  bitmap: number[];
  captured_at: string;
}

interface MachineParametersDto {
  raw: number[];
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

interface ServiceConsoleViewProps {
  scales: Scale[];
  onStatus: (msg: string) => void;
}

export function ServiceConsoleView({ scales, onStatus }: ServiceConsoleViewProps) {
  const xsScales = scales.filter((s) => s.family === "avery_berkel_xs");
  const [host, setHost] = useState<string>(xsScales[0]?.ip_address ?? "192.168.9.113");
  const [weight, setWeight] = useState<ServiceWeightReading | null>(null);
  const [settings748, setSettings748] = useState<ScaleSettings748 | null>(null);
  const [machineParams, setMachineParams] = useState<MachineParametersDto | null>(null);
  const [diagLog, setDiagLog] = useState<string[]>([]);
  const [scanCode, setScanCode] = useState("1");
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [lcdFrame, setLcdFrame] = useState<LcdFrameDto | null>(null);
  const [liveWeight, setLiveWeight] = useState(false);
  const [liveLcd, setLiveLcd] = useState(false);
  const [weightError, setWeightError] = useState<string | null>(null);
  const [lcdError, setLcdError] = useState<string | null>(null);

  useEffect(() => {
    if (!lcdFrame || !canvasRef.current) {
      return;
    }
    const canvas = canvasRef.current;
    canvas.width = lcdFrame.width;
    canvas.height = lcdFrame.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      return;
    }
    const rgba = decode1BppBitmapToRgba(lcdFrame.bitmap, lcdFrame.width, lcdFrame.height);
    const img = new ImageData(new Uint8ClampedArray(rgba), lcdFrame.width, lcdFrame.height);
    ctx.putImageData(img, 0, 0);
  }, [lcdFrame]);

  useEffect(() => {
    if (!liveWeight) {
      return;
    }
    let cancelled = false;
    const poll = async () => {
      try {
        const reading = await invoke<ServiceWeightReading>("xs_service_read_weight", {
          host,
          port: null,
        });
        if (!cancelled) {
          setWeight(reading);
          setWeightError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setWeightError(String(err));
          setLiveWeight(false);
        }
      }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 600);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [liveWeight, host]);

  useEffect(() => {
    if (!liveLcd) {
      return;
    }
    let cancelled = false;
    const poll = async () => {
      try {
        const frame = await invoke<LcdFrameDto>("xs_service_read_lcd", {
          host,
          port: null,
        });
        if (!cancelled) {
          setLcdFrame(frame);
          setLcdError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setLcdError(String(err));
          setLiveLcd(false);
        }
      }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 1200);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [liveLcd, host]);

  const runSafe = async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (err) {
      onStatus(`Service error: ${String(err)}`);
    }
  };

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-card p-3">
        <Wrench className="h-4 w-4 text-primary" />
        <select
          value={host}
          onChange={(ev) => setHost(ev.target.value)}
          className="rounded-md border border-border bg-background px-2 py-1 text-sm"
        >
          {xsScales.map((s) => (
            <option key={s.id} value={s.ip_address}>
              {s.name} ({s.ip_address})
            </option>
          ))}
          {!xsScales.some((s) => s.ip_address === host) && <option value={host}>{host}</option>}
        </select>
        <input
          type="text"
          value={host}
          onChange={(ev) => setHost(ev.target.value)}
          placeholder="XS IP (e.g. 192.168.9.113)"
          className="w-44 rounded-md border border-border bg-background px-2 py-1 text-sm"
        />
        <Badge variant="default">Port 28822 + 3001</Badge>
      </div>

      {/* Live Weight + Hardware Actions */}
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div className="space-y-3 rounded-md border border-border bg-card p-4">
          <div className="flex items-center justify-between gap-2">
            <h3 className="flex items-center gap-2 text-sm font-semibold">
              <ScaleIcon className="h-4 w-4" /> Live Weight & Hardware
            </h3>
            <div className="flex gap-1.5">
              <Button
                size="sm"
                variant={liveWeight ? "destructive" : "default"}
                onClick={() => setLiveWeight((v) => !v)}
              >
                {liveWeight ? "Stop Live" : "Start Live"}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  void runSafe(async () => {
                    const reading = await invoke<ServiceWeightReading>("xs_service_read_weight", {
                      host,
                      port: null,
                    });
                    setWeight(reading);
                    setWeightError(null);
                  })
                }
              >
                <RefreshCw className="h-3.5 w-3.5" /> Read
              </Button>
            </div>
          </div>
          {weightError && <p className="text-xs text-destructive">{weightError}</p>}
          <div className="rounded bg-muted p-3 font-mono text-2xl font-bold">
            {weight ? `${weight.weight_display} kg` : "— . — — —"}
            {weight?.status && (
              <span className="ml-3 text-xs font-normal text-muted-foreground">
                ({weight.status})
              </span>
            )}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                void runSafe(async () => {
                  const res = await invoke<string>("xs_service_balance", { host, port: null });
                  onStatus(`Balance: ${res}`);
                })
              }
            >
              Balance / Zero
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                void runSafe(async () => {
                  await invoke("xs_service_action", { host, port: null, action: "print_test" });
                  onStatus("Test report sent to printer.");
                })
              }
            >
              <Printer className="h-3.5 w-3.5" /> Test Print
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                void runSafe(async () => {
                  await invoke("xs_service_action", { host, port: null, action: "cash_drawer" });
                  onStatus("Cash drawer pulse sent.");
                })
              }
            >
              Cash Drawer
            </Button>
            <Button
              size="sm"
              variant="destructive"
              onClick={() =>
                void runSafe(async () => {
                  await invoke("xs_service_action", { host, port: null, action: "reboot" });
                  onStatus("Reboot command sent to scale.");
                })
              }
            >
              <Power className="h-3.5 w-3.5" /> Reboot
            </Button>
          </div>
        </div>

        {/* Live LCD Mirror + Remote Keypad */}
        <div className="space-y-3 rounded-md border border-border bg-card p-4">
          <div className="flex items-center justify-between gap-2">
            <h3 className="flex items-center gap-2 text-sm font-semibold">
              <Monitor className="h-4 w-4" /> LCD Mirror (320×128) & Remote Key
            </h3>
            <div className="flex gap-1.5">
              <Button
                size="sm"
                variant={liveLcd ? "destructive" : "default"}
                onClick={() => setLiveLcd((v) => !v)}
              >
                {liveLcd ? "Stop Mirror" : "Live Mirror"}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  void runSafe(async () => {
                    const frame = await invoke<LcdFrameDto>("xs_service_read_lcd", {
                      host,
                      port: null,
                    });
                    setLcdFrame(frame);
                    setLcdError(null);
                  })
                }
              >
                Capture
              </Button>
            </div>
          </div>
          {lcdError && <p className="text-xs text-destructive">{lcdError}</p>}
          <canvas
            ref={canvasRef}
            width={320}
            height={128}
            className="w-full max-w-[320px] rounded border border-border bg-lime-100"
          />
          <div className="flex items-center gap-2 text-sm">
            <input
              type="number"
              value={scanCode}
              onChange={(ev) => setScanCode(ev.target.value)}
              className="w-24 rounded-md border border-border bg-background px-2 py-1"
              placeholder="ScanCode"
            />
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                void runSafe(async () => {
                  const code = Number.parseInt(scanCode, 10) || 0;
                  await invoke("xs_service_send_key", {
                    host,
                    port: null,
                    scanCode: code,
                    keyState: 1,
                  });
                  onStatus(`Sent keystroke ${code}.`);
                })
              }
            >
              Send Keystroke
            </Button>
          </div>
        </div>
      </div>

      {/* #748 Scale Settings */}
      <div className="space-y-3 rounded-md border border-border bg-card p-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold">#748 Scale Settings (CMD 62/63)</h3>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                void runSafe(async () => {
                  const cfg = await invoke<ScaleSettings748>("xs_service_read_settings", {
                    host,
                    port: null,
                  });
                  setSettings748(cfg);
                  onStatus("Loaded #748 Scale Settings.");
                })
              }
            >
              Read #748
            </Button>
            {settings748 && (
              <Button
                size="sm"
                onClick={() =>
                  void runSafe(async () => {
                    await invoke("xs_service_write_settings", {
                      host,
                      port: null,
                      settings: settings748,
                    });
                    onStatus("Saved #748 Scale Settings.");
                  })
                }
              >
                Write #748
              </Button>
            )}
          </div>
        </div>
        {settings748 && (
          <div className="grid grid-cols-1 gap-2 font-mono text-xs md:grid-cols-4">
            {(["bank1", "bank2", "bank3", "bank4"] as const).map((bank) => (
              <label key={bank} className="flex flex-col gap-1">
                <span>{bank.toUpperCase()}</span>
                <input
                  value={settings748[bank]}
                  onChange={(ev) => setSettings748({ ...settings748, [bank]: ev.target.value })}
                  className="rounded border border-border bg-background px-2 py-1"
                />
              </label>
            ))}
          </div>
        )}
      </div>

      {/* MACHINE_PARAMETERS (80 B safe editor) */}
      <div className="space-y-3 rounded-md border border-border bg-card p-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold">MACHINE_PARAMETERS (CMD 24 — 80 B Safe Editor)</h3>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                void runSafe(async () => {
                  const params = await invoke<MachineParametersDto>("xs_read_machine_params", {
                    host,
                    port: null,
                  });
                  setMachineParams(params);
                  onStatus("Loaded 80-byte MACHINE_PARAMETERS.");
                })
              }
            >
              Read Params
            </Button>
            {machineParams && (
              <Button
                size="sm"
                onClick={() =>
                  void runSafe(async () => {
                    await invoke("xs_write_machine_params", {
                      host,
                      port: null,
                      params: machineParams,
                    });
                    onStatus("Wrote 80-byte MACHINE_PARAMETERS safely.");
                  })
                }
              >
                Write Params
              </Button>
            )}
          </div>
        </div>
        {machineParams && (
          <div className="grid grid-cols-2 gap-3 text-sm md:grid-cols-4">
            <label className="flex flex-col gap-1">
              <span>Print Mode (PTM)</span>
              <select
                value={machineParams.print_mode}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, print_mode: Number(ev.target.value) })
                }
                className="rounded border border-border bg-background px-2 py-1"
              >
                <option value={0}>0 — On Request</option>
                <option value={1}>1 — Immediate</option>
                <option value={2}>2 — Pre-Pack</option>
              </select>
            </label>
            <label className="flex flex-col gap-1">
              <span>Label Speed (%)</span>
              <input
                type="number"
                value={machineParams.label_print_speed}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, label_print_speed: Number(ev.target.value) })
                }
                className="rounded border border-border bg-background px-2 py-1"
              />
            </label>
            <label className="flex items-center gap-2 pt-5">
              <input
                type="checkbox"
                checked={machineParams.self_service_mode}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, self_service_mode: ev.target.checked })
                }
              />
              <span>Self Service (SSM)</span>
            </label>
            <label className="flex items-center gap-2 pt-5">
              <input
                type="checkbox"
                checked={machineParams.tear_bar_fitted}
                onChange={(ev) =>
                  setMachineParams({ ...machineParams, tear_bar_fitted: ev.target.checked })
                }
              />
              <span>Tear Bar (TBRF)</span>
            </label>
          </div>
        )}
      </div>

      {/* Diagnostic Logs */}
      <div className="space-y-2 rounded-md border border-border bg-card p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-sm font-semibold">Diagnostic Logs (CMD 52)</h3>
          <div className="flex gap-1.5">
            {(["SYSTEM", "COMMS", "KEYBOARD"] as const).map((kind) => (
              <Button
                key={kind}
                size="sm"
                variant="outline"
                onClick={() =>
                  void runSafe(async () => {
                    const lines = await invoke<string[]>("xs_service_read_diag_log", {
                      host,
                      port: null,
                      logType: kind,
                    });
                    setDiagLog(lines);
                    onStatus(`Loaded ${lines.length} ${kind} log frame(s).`);
                  })
                }
              >
                {kind}
              </Button>
            ))}
          </div>
        </div>
        {diagLog.length > 0 && (
          <pre className="max-h-48 overflow-auto rounded bg-muted p-3 text-xs">
            {diagLog.join("\n")}
          </pre>
        )}
      </div>
    </div>
  );
}
