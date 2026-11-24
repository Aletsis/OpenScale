import { afterEach, describe, expect, it, vi } from "vitest";

import { HttpWsTransport } from "./transport.js";
import type { Plu, Scale, ScaleEvent } from "./types.js";

function mockJsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

describe("HttpWsTransport", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("strips trailing slashes and attaches API key and Bearer headers", async () => {
    const fetchMock = vi.fn().mockResolvedValue(mockJsonResponse([]));
    vi.stubGlobal("fetch", fetchMock);

    const transport = new HttpWsTransport({
      baseUrl: "http://127.0.0.1:8080///",
      apiKey: "osk_test_123",
      bearerToken: "jwt_abc",
    });

    const scales = await transport.listScales();
    expect(scales).toEqual([]);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:8080/api/v1/scales",
      expect.objectContaining({
        headers: expect.objectContaining({
          "Content-Type": "application/json",
          "X-API-Key": "osk_test_123",
          Authorization: "Bearer jwt_abc",
        }),
      }),
    );
  });

  it("calls discoverScales, upsertScale, listPlus, getPlu, upsertPlu, and deletePlu", async () => {
    const sampleScale: Scale = {
      id: "scale-1",
      group_id: null,
      name: "XS 01",
      family: "avery_berkel_xs",
      ip_address: "192.168.9.113",
      data_port: 3001,
      service_port: 28822,
      serial_number: "22320310",
      model: "XS200",
      firmware_version: "APP. v4.1.7.0",
      mac_address: null,
      status: "online",
      last_seen_at: null,
      meta: {
        store_id: "store-1",
        sync_version: 1,
        updated_at: "2026-11-22T00:00:00Z",
        deleted_at: null,
      },
    };
    const samplePlu: Plu = {
      id: "plu-1",
      key: { did: 1, plu: 100 },
      description: "POLLO FRESCO",
      button_text: null,
      price_1: 4500,
      price_2: 0,
      price_3: 0,
      product_type: "by_weight",
      ean: "2000100000000",
      sku: null,
      group_id: 0,
      barcode_format_id: 1,
      label_format_id: 1,
      tare_id: 0,
      tare_grams: null,
      date_offset_1: 5,
      date_print_control_1: 1,
      date_offset_2: 0,
      date_print_control_2: 0,
      tax_rate_id_1: 0,
      tax_rate_id_2: 0,
      text_areas: [],
      delta_hash: "abc",
      meta: sampleScale.meta,
    };

    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(mockJsonResponse([]))
      .mockResolvedValueOnce(mockJsonResponse(sampleScale))
      .mockResolvedValueOnce(mockJsonResponse(null))
      .mockResolvedValueOnce(mockJsonResponse([samplePlu]))
      .mockResolvedValueOnce(mockJsonResponse(samplePlu))
      .mockResolvedValueOnce(mockJsonResponse(samplePlu))
      .mockResolvedValueOnce(mockJsonResponse(null));
    vi.stubGlobal("fetch", fetchMock);

    const transport = new HttpWsTransport({ baseUrl: "http://localhost:8080" });

    expect(await transport.discoverScales()).toEqual([]);
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:8080/api/v1/scales/discover",
      expect.objectContaining({ method: "POST" }),
    );

    expect(await transport.upsertScale(sampleScale)).toEqual(sampleScale);
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:8080/api/v1/scales/scale-1",
      expect.objectContaining({ method: "PUT", body: JSON.stringify(sampleScale) }),
    );

    expect(await transport.deleteScale("scale-1")).toBe(true);
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "http://localhost:8080/api/v1/scales/scale-1",
      expect.objectContaining({ method: "DELETE" }),
    );

    expect(await transport.listPlus()).toEqual([samplePlu]);
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "http://localhost:8080/api/v1/plus",
      expect.objectContaining({
        headers: { "Content-Type": "application/json" },
      }),
    );

    expect(await transport.getPlu({ did: 1, plu: 100 })).toEqual(samplePlu);
    expect(fetchMock).toHaveBeenNthCalledWith(
      5,
      "http://localhost:8080/api/v1/plus/1/100",
      expect.objectContaining({
        headers: { "Content-Type": "application/json" },
      }),
    );

    expect(await transport.upsertPlu(samplePlu)).toEqual(samplePlu);
    expect(fetchMock).toHaveBeenNthCalledWith(
      6,
      "http://localhost:8080/api/v1/plus/1/100",
      expect.objectContaining({ method: "PUT", body: JSON.stringify(samplePlu) }),
    );

    expect(await transport.deletePlu({ did: 1, plu: 100 })).toBe(true);
    expect(fetchMock).toHaveBeenNthCalledWith(
      7,
      "http://localhost:8080/api/v1/plus/1/100",
      expect.objectContaining({ method: "DELETE" }),
    );
  });

  it("calls importFromScale, previewImportFromScale, applyScaleImport, and syncScale endpoints", async () => {
    const report = {
      scale_id: "scale-1",
      mode: "delta" as const,
      total_catalog_plus: 10,
      sent_plus: 2,
      skipped_unchanged: 8,
      failed_plus: 0,
    };
    const selection = {
      plus: true,
      departments: true,
      tares: false,
      tax_rates: false,
      stored_texts: false,
      barcode_formats: false,
      label_formats: false,
    };
    const preview = {
      scale_id: "scale-1",
      scale_name: "XS 01",
      previewed_at: "2026-11-23T22:00:00Z",
      plus: [],
      departments: [],
      tares: [],
      tax_rates: [],
      stored_texts: [],
      barcode_formats: [],
      label_formats: [],
      warnings: [],
    };
    const applyRequest = {
      scale_id: "scale-1",
      plus: [],
      departments: [],
      tares: [],
      tax_rates: [],
      stored_texts: [],
      barcode_formats: [],
      label_formats: [],
      kept_local_conflicts: 0,
      skipped_unchanged: 0,
    };
    const applyResult = {
      scale_id: "scale-1",
      imported_plus: 0,
      imported_departments: 0,
      imported_tares: 0,
      imported_tax_rates: 0,
      imported_stored_texts: 0,
      imported_barcode_formats: 0,
      imported_label_formats: 0,
      total_imported: 0,
      kept_local_conflicts: 0,
      skipped_unchanged: 0,
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(mockJsonResponse(740))
      .mockResolvedValueOnce(mockJsonResponse(preview))
      .mockResolvedValueOnce(mockJsonResponse(applyResult))
      .mockResolvedValueOnce(mockJsonResponse(report));
    vi.stubGlobal("fetch", fetchMock);

    const transport = new HttpWsTransport({ baseUrl: "http://localhost:8080" });
    expect(await transport.importFromScale("scale-1")).toBe(740);
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:8080/api/v1/scales/scale-1/import",
      expect.objectContaining({ method: "POST" }),
    );

    expect(await transport.previewImportFromScale("scale-1", selection)).toEqual(preview);
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:8080/api/v1/scales/scale-1/import/preview",
      expect.objectContaining({ method: "POST", body: JSON.stringify(selection) }),
    );

    expect(await transport.applyScaleImport(applyRequest)).toEqual(applyResult);
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "http://localhost:8080/api/v1/scales/scale-1/import/apply",
      expect.objectContaining({ method: "POST", body: JSON.stringify(applyRequest) }),
    );

    expect(await transport.syncScale("scale-1", "delta")).toEqual(report);
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "http://localhost:8080/api/v1/scales/scale-1/sync",
      expect.objectContaining({ method: "POST", body: JSON.stringify({ mode: "delta" }) }),
    );
  });

  it("calls checkScaleHealth and checkAllScalesHealth endpoints", async () => {
    const result = {
      scale_id: "scale-1",
      previous_status: "online" as const,
      status: "offline" as const,
      last_seen_at: "2026-11-23T22:00:00Z",
      checked_at: "2026-11-23T22:01:00Z",
      error: "Connection timed out",
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(mockJsonResponse(result))
      .mockResolvedValueOnce(mockJsonResponse([result]));
    vi.stubGlobal("fetch", fetchMock);

    const transport = new HttpWsTransport({ baseUrl: "http://localhost:8080" });
    expect(await transport.checkScaleHealth("scale-1")).toEqual(result);
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:8080/api/v1/scales/scale-1/health",
      expect.objectContaining({ method: "POST" }),
    );
    expect(await transport.checkAllScalesHealth()).toEqual([result]);
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:8080/api/v1/scales/health",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("throws a descriptive error when API responds with non-OK status", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response("Forbidden", { status: 403 }));
    vi.stubGlobal("fetch", fetchMock);

    const transport = new HttpWsTransport({ baseUrl: "http://localhost:8080" });
    await expect(transport.listScales()).rejects.toThrow("OpenScale API error (403): Forbidden");
  });

  it("subscribes to WebSocket events, ignores malformed frames, and closes on unsubscribe", () => {
    const wsState: { onMessage?: (ev: { data: string }) => void } = {};
    const closeSpy = vi.fn();
    let constructedUrl = "";

    class FakeWebSocket {
      constructor(url: string) {
        constructedUrl = url;
      }
      addEventListener(type: string, handler: (ev: { data: string }) => void) {
        if (type === "message") {
          wsState.onMessage = handler;
        }
      }
      close() {
        closeSpy();
      }
    }

    vi.stubGlobal("WebSocket", FakeWebSocket);

    const transport = new HttpWsTransport({ baseUrl: "https://scales.local" });
    const received: ScaleEvent[] = [];
    const unsubscribe = transport.subscribe((ev) => received.push(ev));

    expect(constructedUrl).toBe("wss://scales.local/api/v1/ws");

    const event: ScaleEvent = {
      type: "connection_changed",
      scale_id: "scale-1",
      state: "connected",
    };
    wsState.onMessage?.({ data: JSON.stringify(event) });
    wsState.onMessage?.({ data: "{invalid json" });

    expect(received).toEqual([event]);

    unsubscribe();
    expect(closeSpy).toHaveBeenCalledTimes(1);

    const wsTransport = new HttpWsTransport({ baseUrl: "wss://http-gateway.local" });
    const unsub2 = wsTransport.subscribe(() => {});
    expect(constructedUrl).toBe("wss://http-gateway.local/api/v1/ws");
    unsub2();
  });
});
