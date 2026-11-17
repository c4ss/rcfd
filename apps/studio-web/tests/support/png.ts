// A small PNG reader for the tests (F-808): 8-bit RGB or RGBA, not interlaced, which is what Chromium's screenshots are.
// Node's zlib inflates; the five scanline filters of the PNG specification are undone here. No dependency.
import { inflateSync } from 'node:zlib';

export interface Rgba {
  readonly width: number;
  readonly height: number;
  /** Row-major from the top, 4 bytes per pixel. */
  readonly data: Uint8Array;
}

export function decodePng(png: Uint8Array): Rgba {
  const sig = [137, 80, 78, 71, 13, 10, 26, 10];
  if (!sig.every((b, i) => png[i] === b)) throw new Error('not a PNG');
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength);
  let pos = 8;
  let width = 0;
  let height = 0;
  let channels = 0;
  const idat: Uint8Array[] = [];
  while (pos < png.length) {
    const len = view.getUint32(pos);
    const type = String.fromCharCode(...png.subarray(pos + 4, pos + 8));
    const body = png.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') {
      const b = new DataView(body.buffer, body.byteOffset, body.byteLength);
      width = b.getUint32(0);
      height = b.getUint32(4);
      const depth = body[8]!;
      const colour = body[9]!;
      const interlace = body[12]!;
      if (depth !== 8 || interlace !== 0 || (colour !== 2 && colour !== 6)) throw new Error(`PNG kind not read here: depth ${depth}, colour ${colour}, interlace ${interlace}`);
      channels = colour === 6 ? 4 : 3;
    } else if (type === 'IDAT') idat.push(body);
    else if (type === 'IEND') break;
    pos += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const out = new Uint8Array(width * height * 4);
  let prev = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)]!;
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const cur = new Uint8Array(stride);
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? cur[i - channels]! : 0;
      const b = prev[i]!;
      const c = i >= channels ? prev[i - channels]! : 0;
      let v = line[i]!;
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      cur[i] = v & 255;
    }
    for (let x = 0; x < width; x++) {
      const o = 4 * (y * width + x);
      out[o] = cur[x * channels]!;
      out[o + 1] = cur[x * channels + 1]!;
      out[o + 2] = cur[x * channels + 2]!;
      out[o + 3] = channels === 4 ? cur[x * channels + 3]! : 255;
    }
    prev = cur;
  }
  return { width, height, data: out };
}

/** The pixel at (x, y) as [r, g, b]. */
export function pixel(img: Rgba, x: number, y: number): [number, number, number] {
  const o = 4 * (Math.round(y) * img.width + Math.round(x));
  return [img.data[o]!, img.data[o + 1]!, img.data[o + 2]!];
}

/** True when every pixel within `r` of (x, y) has the same colour (inside one flat triangle, away from its edges). */
export function uniformAround(img: Rgba, x: number, y: number, r: number): boolean {
  const c = pixel(img, x, y);
  for (let dy = -r; dy <= r; dy++) {
    for (let dx = -r; dx <= r; dx++) {
      const q = pixel(img, x + dx, y + dy);
      if (q[0] !== c[0] || q[1] !== c[1] || q[2] !== c[2]) return false;
    }
  }
  return true;
}
