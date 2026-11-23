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

    expect(await transport.listPlus()).toEqual([samplePlu]);
    expect(await transport.getPlu({ did: 1, plu: 100 })).toEqual(samplePlu);
    expect(await transport.upsertPlu(samplePlu)).toEqual(samplePlu);
    expect(await transport.deletePlu({ did: 1, plu: 100 })).toBe(true);
    expect(fetchMock).toHaveBeenNthCalledWith(
      6,
      "http://localhost:8080/api/v1/plus/1/100",
      expect.objectContaining({ method: "DELETE" }),
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
  });
});
