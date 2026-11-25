import type {
  Branch,
  DiscoveredScale,
  ImportSelection,
  OpenScaleTransport,
  Plu,
  PluKey,
  Scale,
  ScaleEvent,
  ScaleHealthCheckResult,
  ScaleImportApplyRequest,
  ScaleImportPreview,
  ScaleImportResult,
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
  listBranches(): Promise<Branch[]> {
    return invoke<Branch[]>("list_branches");
  }

  upsertBranch(branch: Branch): Promise<Branch> {
    return invoke<Branch>("upsert_branch", { branch });
  }

  deleteBranch(branchId: string): Promise<boolean> {
    return invoke<boolean>("delete_branch", { branchId });
  }

  listBranchPluKeys(branchId: string): Promise<PluKey[]> {
    return invoke<PluKey[]>("list_branch_plu_keys", { branchId });
  }

  setBranchPluKeys(branchId: string, keys: PluKey[]): Promise<number> {
    return invoke<number>("set_branch_plu_keys", { branchId, keys });
  }

  discoverScales(): Promise<DiscoveredScale[]> {
    return invoke<DiscoveredScale[]>("discover_scales");
  }

  listScales(): Promise<Scale[]> {
    return invoke<Scale[]>("list_scales");
  }

  upsertScale(scale: Scale): Promise<Scale> {
    return invoke<Scale>("upsert_scale", { scale });
  }

  deleteScale(scaleId: string): Promise<boolean> {
    return invoke<boolean>("delete_scale", { scaleId });
  }

  checkScaleHealth(scaleId: string): Promise<ScaleHealthCheckResult> {
    return invoke<ScaleHealthCheckResult>("check_scale_health", { scaleId });
  }

  checkAllScalesHealth(): Promise<ScaleHealthCheckResult[]> {
    return invoke<ScaleHealthCheckResult[]>("check_all_scales_health");
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

  previewImportFromScale(scaleId: string, selection: ImportSelection): Promise<ScaleImportPreview> {
    return invoke<ScaleImportPreview>("preview_import_from_scale", { scaleId, selection });
  }

  applyScaleImport(request: ScaleImportApplyRequest): Promise<ScaleImportResult> {
    return invoke<ScaleImportResult>("apply_scale_import", { request });
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
