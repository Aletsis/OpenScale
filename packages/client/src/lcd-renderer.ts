/**
 * Decodes a 1-bpp packed monochrome LCD bitmap (e.g. XS 320x128 -> 5120 bytes)
 * into an RGBA `Uint8ClampedArray` suitable for HTML5 `<canvas>` `ImageData`.
 */
export function decode1BppBitmapToRgba(
  bitmap: readonly number[] | Uint8Array,
  width: number,
  height: number,
  onColor: readonly [number, number, number, number] = [15, 23, 42, 255],
  offColor: readonly [number, number, number, number] = [217, 249, 157, 255],
): Uint8ClampedArray {
  const totalPixels = Math.max(0, width * height);
  const rgba = new Uint8ClampedArray(totalPixels * 4);

  for (let pixel = 0; pixel < totalPixels; pixel++) {
    const byteIndex = pixel >> 3;
    const bitMask = 0x80 >> (pixel & 7);
    const byteVal = bitmap[byteIndex] ?? 0;
    const isOn = (byteVal & bitMask) !== 0;
    const color = isOn ? onColor : offColor;

    const out = pixel * 4;
    rgba[out] = color[0];
    rgba[out + 1] = color[1];
    rgba[out + 2] = color[2];
    rgba[out + 3] = color[3];
  }

  return rgba;
}
