import type { Grid, Row } from '../types'

const BARS = '▁▂▃▄▅▆▇█'

// Sparkline of log10(values) over the last `width` samples.
export function spark(values: number[], width: number): string {
  const v = values.slice(-width).map(x => Math.log10(Math.max(x, 1e-300)))
  if (v.length === 0) return ''
  const lo = Math.min(...v)
  const hi = Math.max(...v)
  return v.map(x => BARS[hi > lo ? Math.min(7, Math.floor(((x - lo) / (hi - lo)) * 8)) : 0]).join('')
}

// Viridis-like ramp, t in [0, 1] -> 0xRRGGBB.
export function ramp(t: number): number {
  const stops = [
    [68, 1, 84],
    [59, 82, 139],
    [33, 145, 140],
    [94, 201, 98],
    [253, 231, 37],
  ]
  const x = Math.min(1, Math.max(0, t)) * (stops.length - 1)
  const i = Math.min(stops.length - 2, Math.floor(x))
  const f = x - i
  const c = stops[i].map((a, k) => Math.round(a + (stops[i + 1][k] - a) * f))
  return (c[0] << 16) | (c[1] << 8) | c[2]
}

// Base64 of a grid as terminal Raster cells (full blocks coloured by value).
export function gridCells(g: Grid): string {
  const words = new Uint32Array(g.cols * g.rows * 3)
  const span = g.max > g.min ? g.max - g.min : 1
  g.values.forEach((v, i) => {
    words[i * 3] = v === null ? 0x20 : 0x2588
    words[i * 3 + 1] = v === null ? 0x01000000 : ramp((v - g.min) / span)
    words[i * 3 + 2] = 0x01000000
  })
  return new Uint8Array(words.buffer).toBase64()
}

export type SchemaNode = {
  type?: string
  description?: string
  enum?: string[]
  minimum?: number
  maximum?: number
  properties?: Record<string, SchemaNode>
  additionalProperties?: SchemaNode | boolean
}

// Flatten the settings tree into editable rows, taking kind and help from the JSON Schema.
export function flatten(tree: unknown, schema: SchemaNode | undefined, path: string, out: Row[]): void {
  if (tree !== null && typeof tree === 'object' && !Array.isArray(tree)) {
    for (const [k, v] of Object.entries(tree as Record<string, unknown>)) {
      const child =
        schema?.properties?.[k] ??
        (typeof schema?.additionalProperties === 'object' ? schema.additionalProperties : undefined)
      flatten(v, child, path ? `${path}.${k}` : k, out)
    }
    return
  }
  const bounds =
    schema?.minimum !== undefined || schema?.maximum !== undefined
      ? `[${schema?.minimum ?? ''}, ${schema?.maximum ?? ''}]`
      : ''
  out.push({
    path,
    value: JSON.stringify(tree),
    kind: schema?.type ?? typeof tree,
    doc: schema?.description ?? '',
    choices: schema?.enum ? schema.enum.join(' | ') : bounds,
  })
}

// Text typed into the editor -> JSON value, guided by the row's kind.
export function parseValue(text: string, kind: string): unknown {
  const t = text.trim()
  if (kind === 'string') return t.startsWith('"') ? JSON.parse(t) : t
  return JSON.parse(t)
}

