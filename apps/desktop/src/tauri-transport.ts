import type {
  DiscoveredScale,
  OpenScaleTransport,
  Plu,
  PluKey,
  Scale,
  ScaleEvent,
  SyncMode,
  SyncReport,
  Unsubscribe,
} from "@openscale/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Phase 1 Desktop Standalone transport powered by Tauri v2 IPC commands.
 */
export class TauriIpcTransport implements OpenScaleTransport {
  discoverScales(): Promise<DiscoveredScale[]> {
    return invoke<DiscoveredScale[]>("discover_scales");
  }

  listScales(): Promise<Scale[]> {
    return invoke<Scale[]>("list_scales");
  }

  upsertScale(scale: Scale): Promise<Scale> {
    return invoke<Scale>("upsert_scale", { scale });
  }

  listPlus(): Promise<Plu[]> {
    return invoke<Plu[]>("list_plus");
  }

  getPlu(key: PluKey): Promise<Plu | null> {
    return invoke<Plu | null>("get_plu", { key });
  }

  upsertPlu(plu: Plu): Promise<Plu> {
    return invoke<Plu>("upsert_plu", { plu });
  }

  deletePlu(key: PluKey): Promise<boolean> {
    return invoke<boolean>("delete_plu", { key });
  }

  importFromScale(scaleId: string): Promise<number> {
    return invoke<number>("import_from_scale", { scaleId });
  }

  syncScale(scaleId: string, mode: SyncMode): Promise<SyncReport> {
    return invoke<SyncReport>("sync_scale", { scaleId, mode });
  }

  subscribe(listener: (event: ScaleEvent) => void): Unsubscribe {
    let unlisten: (() => void) | null = null;
    let disposed = false;

    void listen<ScaleEvent>("openscale://event", (ev) => {
      listener(ev.payload);
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }
}
