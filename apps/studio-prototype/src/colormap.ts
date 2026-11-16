import * as THREE from 'three/webgpu';

// Viridis (van der Walt and Smith, matplotlib, CC0), sampled at 9 control points and interpolated linearly into a
// 256-entry table. The same table feeds the GPU texture and the CSS legend, so the legend is the map.
export const VIRIDIS_STOPS = [
  '#440154', '#472d7b', '#3b528b', '#2c728e', '#21918c', '#28ae80', '#5ec962', '#addc30', '#fde725',
];

function rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function viridisLUT(): THREE.DataTexture {
  const N = 256;
  const data = new Uint8Array(N * 4);
  const stops = VIRIDIS_STOPS.map(rgb);
  for (let i = 0; i < N; i++) {
    const x = (i / (N - 1)) * (stops.length - 1);
    const k = Math.min(stops.length - 2, Math.floor(x));
    const f = x - k;
    for (let c = 0; c < 3; c++) data[i * 4 + c] = Math.round(stops[k][c] * (1 - f) + stops[k + 1][c] * f);
    data[i * 4 + 3] = 255;
  }
  const tex = new THREE.DataTexture(data, N, 1, THREE.RGBAFormat);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearFilter;
  tex.needsUpdate = true;
  return tex;
}

export const VIRIDIS_CSS = `linear-gradient(to top, ${VIRIDIS_STOPS.join(', ')})`;
