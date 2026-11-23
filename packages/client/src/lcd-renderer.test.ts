import { describe, expect, it } from "vitest";

import { decode1BppBitmapToRgba } from "./lcd-renderer.js";

describe("decode1BppBitmapToRgba", () => {
  it("decodes packed 1-bpp MSB-first pixels into RGBA bytes", () => {
    // 0b1010_0000 -> pixels 0 and 2 ON, rest OFF
    const rgba = decode1BppBitmapToRgba([0b1010_0000], 8, 1, [0, 0, 0, 255], [255, 255, 255, 255]);
    expect(rgba.length).toBe(8 * 4);
    expect(Array.from(rgba.slice(0, 4))).toEqual([0, 0, 0, 255]);
    expect(Array.from(rgba.slice(4, 8))).toEqual([255, 255, 255, 255]);
    expect(Array.from(rgba.slice(8, 12))).toEqual([0, 0, 0, 255]);
    expect(Array.from(rgba.slice(12, 16))).toEqual([255, 255, 255, 255]);
  });

  it("handles short bitmaps safely using default colors", () => {
    const rgba = decode1BppBitmapToRgba([], 4, 1);
    expect(rgba.length).toBe(16);
    expect(rgba[3]).toBe(255);
  });
});
