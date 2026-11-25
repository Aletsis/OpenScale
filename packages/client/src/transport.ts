import type {
  Branch,
  DeltaSyncPreview,
  DiscoveredScale,
  DiscoveredScaleMatch,
  ImportSelection,
  Plu,
  PluKey,
  PriceChangeItem,
  PriceChangeReport,
  Scale,
  ScaleEvent,
  ScaleHealthCheckResult,
  ScaleImportApplyRequest,
  ScaleImportPreview,
  ScaleImportResult,
  SyncMode,
  SyncReport,
} from "./types.js";

export type Unsubscribe = () => void;

/**
 * Transport abstraction allowing 100% UI reuse between:
 * - Phase 1 Desktop Standalone (`TauriIpcTransport`)
 * - Phase 2+ Desktop Remote & Phase 4 Web App (`HttpWsTransport`)
 */
export interface OpenScaleTransport {
  // Branches
  listBranches?(): Promise<Branch[]>;
  upsertBranch?(branch: Branch): Promise<Branch>;
  deleteBranch?(branchId: string): Promise<boolean>;
  listBranchPluKeys?(branchId: string): Promise<PluKey[]>;
  setBranchPluKeys?(branchId: string, keys: PluKey[]): Promise<number>;

  // Scales
  discoverScales(): Promise<DiscoveredScale[]>;
  discoverScalesWithIdentity?(): Promise<DiscoveredScaleMatch[]>;
  reassignScaleIp?(scaleId: string, newIp: string): Promise<Scale>;
  swapScaleIps?(scaleAId: string, scaleBId: string): Promise<[Scale, Scale]>;
  listScales(): Promise<Scale[]>;
  upsertScale(scale: Scale): Promise<Scale>;
  deleteScale(scaleId: string): Promise<boolean>;
  checkScaleHealth(scaleId: string): Promise<ScaleHealthCheckResult>;
  checkAllScalesHealth(): Promise<ScaleHealthCheckResult[]>;

  // Master PLU Catalog
  listPlus(): Promise<Plu[]>;
  getPlu(key: PluKey): Promise<Plu | null>;
  upsertPlu(plu: Plu): Promise<Plu>;
  deletePlu(key: PluKey): Promise<boolean>;

  // Synchronization
  importFromScale(scaleId: string): Promise<number>;
  previewImportFromScale(scaleId: string, selection: ImportSelection): Promise<ScaleImportPreview>;
  applyScaleImport(request: ScaleImportApplyRequest): Promise<ScaleImportResult>;
  syncScale(scaleId: string, mode: SyncMode): Promise<SyncReport>;
  previewDeltaSync?(scaleId: string): Promise<DeltaSyncPreview>;
  syncScaleWithOptions?(
    scaleId: string,
    mode: SyncMode,
    includeDeletes: boolean,
  ): Promise<SyncReport>;
  applyPriceChanges?(
    items: PriceChangeItem[],
    targetScaleIds: string[],
    sendImmediately: boolean,
  ): Promise<PriceChangeReport>;

  // Real-time events
  subscribe(listener: (event: ScaleEvent) => void): Unsubscribe;
}

export interface HttpWsTransportOptions {
  baseUrl: string;
  apiKey?: string;
  bearerToken?: string;
}

/**
 * REST + WebSocket transport targeting `openscaled` (Phase 2+).
 */
export class HttpWsTransport implements OpenScaleTransport {
  private readonly baseUrl: string;
  private readonly headers: Record<string, string>;

  constructor(options: HttpWsTransportOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    this.headers = {
      "Content-Type": "application/json",
    };
    if (options.apiKey) {
      this.headers["X-API-Key"] = options.apiKey;
    }
    if (options.bearerToken) {
      this.headers.Authorization = `Bearer ${options.bearerToken}`;
    }
  }

  private async request<T>(path: string, init?: RequestInit): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: {
        ...this.headers,
        ...(init?.headers ?? {}),
      },
    });
    if (!response.ok) {
      throw new Error(`OpenScale API error (${response.status}): ${await response.text()}`);
    }
    return (await response.json()) as T;
  }

  discoverScales(): Promise<DiscoveredScale[]> {
    return this.request<DiscoveredScale[]>("/api/v1/scales/discover", { method: "POST" });
  }

  listScales(): Promise<Scale[]> {
    return this.request<Scale[]>("/api/v1/scales");
  }

  upsertScale(scale: Scale): Promise<Scale> {
    return this.request<Scale>(`/api/v1/scales/${encodeURIComponent(scale.id)}`, {
      method: "PUT",
      body: JSON.stringify(scale),
    });
  }

  async deleteScale(scaleId: string): Promise<boolean> {
    await this.request<void>(`/api/v1/scales/${encodeURIComponent(scaleId)}`, {
      method: "DELETE",
    });
    return true;
  }

  checkScaleHealth(scaleId: string): Promise<ScaleHealthCheckResult> {
    return this.request<ScaleHealthCheckResult>(
      `/api/v1/scales/${encodeURIComponent(scaleId)}/health`,
      { method: "POST" },
    );
  }

  checkAllScalesHealth(): Promise<ScaleHealthCheckResult[]> {
    return this.request<ScaleHealthCheckResult[]>("/api/v1/scales/health", { method: "POST" });
  }

  listPlus(): Promise<Plu[]> {
    return this.request<Plu[]>("/api/v1/plus");
  }

  getPlu(key: PluKey): Promise<Plu | null> {
    return this.request<Plu | null>(`/api/v1/plus/${key.did}/${key.plu}`);
  }

  upsertPlu(plu: Plu): Promise<Plu> {
    return this.request<Plu>(`/api/v1/plus/${plu.key.did}/${plu.key.plu}`, {
      method: "PUT",
      body: JSON.stringify(plu),
    });
  }

  async deletePlu(key: PluKey): Promise<boolean> {
    await this.request<void>(`/api/v1/plus/${key.did}/${key.plu}`, { method: "DELETE" });
    return true;
  }

  importFromScale(scaleId: string): Promise<number> {
    return this.request<number>(`/api/v1/scales/${encodeURIComponent(scaleId)}/import`, {
      method: "POST",
    });
  }

  previewImportFromScale(scaleId: string, selection: ImportSelection): Promise<ScaleImportPreview> {
    return this.request<ScaleImportPreview>(
      `/api/v1/scales/${encodeURIComponent(scaleId)}/import/preview`,
      {
        method: "POST",
        body: JSON.stringify(selection),
      },
    );
  }

  applyScaleImport(request: ScaleImportApplyRequest): Promise<ScaleImportResult> {
    return this.request<ScaleImportResult>(
      `/api/v1/scales/${encodeURIComponent(request.scale_id)}/import/apply`,
      {
        method: "POST",
        body: JSON.stringify(request),
      },
    );
  }

  syncScale(scaleId: string, mode: SyncMode): Promise<SyncReport> {
    return this.request<SyncReport>(`/api/v1/scales/${encodeURIComponent(scaleId)}/sync`, {
      method: "POST",
      body: JSON.stringify({ mode }),
    });
  }

  subscribe(listener: (event: ScaleEvent) => void): Unsubscribe {
    const wsUrl = `${this.baseUrl.replace(/^http/, "ws")}/api/v1/ws`;
    const socket = new WebSocket(wsUrl);
    socket.addEventListener("message", (msg) => {
      try {
        listener(JSON.parse(String(msg.data)) as ScaleEvent);
      } catch {
        // Ignore malformed frames
      }
    });
    return () => {
      socket.close();
    };
  }
}
