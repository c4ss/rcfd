import { atom, read, update } from 'claude-code'
import type { Register } from 'claude-code'

import type { Row, Snapshot } from '../types'

import { flatten, gridCells, parseValue, spark } from './model'
import type { SchemaNode } from './model'

// rcfd sidebar: schema-driven settings editor (F-092) and live monitors (F-093).
// It talks to a local `rcfd serve` through `rcfd call` (JSON-RPC, protocol rcfd-service/1).

const PANE = 'rcfd'
const PORT = 7878
const ADDR = `127.0.0.1:${PORT}`
const POLL_MS = 500
const MAX_POINTS = 600

const TEMPLATE = `title = "lid-driven cavity Re=100"
[mesh]
generator = "cavity_2d"
nx = 24
ny = 24
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 2000
tolerance_u = 1e-7
tolerance_p = 1e-7
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
`

const snapA = atom({ plugin: 'rcfd-panel', key: 'snap' } as const, null as Snapshot | null)
const rowsA = atom({ plugin: 'rcfd-panel', key: 'rows' } as const, [] as Row[])
const viewA = atom({ plugin: 'rcfd-panel', key: 'view' } as const, 'monitors' as 'monitors' | 'settings')
const sectionA = atom({ plugin: 'rcfd-panel', key: 'section' } as const, '')
const selectedA = atom({ plugin: 'rcfd-panel', key: 'selected' } as const, '')
const messageA = atom({ plugin: 'rcfd-panel', key: 'message' } as const, '')

let started = false
let session = ''
let schema: SchemaNode | undefined
let busy = false
const series: Record<string, { x: number[]; y: number[] }> = {}
const rcfd = 'rcfd'

async function rpc($: any, method: string, params: Record<string, unknown> = {}): Promise<any> {
  const r = await $.process.run([rcfd, 'call', '--addr', ADDR, method, JSON.stringify(params)], { timeoutMs: 60000 })
  if (r.exitCode !== 0) {
    let msg = (r.stderr || r.stdout || 'rcfd call failed').trim()
    try {
      msg = JSON.parse(msg).message ?? msg
    } catch {
      /* keep text */
    }
    throw new Error(msg)
  }
  return JSON.parse(r.stdout)
}

async function ensureServer($: any): Promise<void> {
  if (started) return
  started = true
  void (async () => {
    try {
      for await (const _ of $.process.spawn({ argv: [rcfd, 'serve', '--addr', ADDR] })) {
        // the child lives as long as this loop; its output is not used
      }
    } catch {
      started = false
    }
  })()
  for (let i = 0; i < 40; i++) {
    try {
      await rpc($, 'service.info')
      return
    } catch {
      await $.clock.sleep(150)
    }
  }
  throw new Error(`rcfd service did not start on ${ADDR} (is rcfd on PATH?)`)
}

async function loadTree($: any): Promise<void> {
  if (!schema) schema = (await rpc($, 'settings.schema')).case
  const tree = (await rpc($, 'settings.get', { session })).value
  const rows: Row[] = []
  flatten(tree, schema, '', rows)
  await update($, rowsA, () => rows)
}

async function newSession($: any): Promise<void> {
  await ensureServer($)
  session = (await rpc($, 'session.create', { toml: TEMPLATE })).session
  for (const k of Object.keys(series)) delete series[k]
  await update($, snapA, () => null)
  await loadTree($)
  await refresh($)
}

async function refresh($: any): Promise<void> {
  if (busy || !session) return
  busy = true
  try {
    const since = series.res_u?.x.length ? series.res_u.x[series.res_u.x.length - 1] : undefined
    const view = await read($, viewA)
    const snap = await rpc($, 'panel.snapshot', {
      session,
      since,
      log_lines: 6,
      ...(view === 'monitors' ? { field: 'speed', cols: 40, rows: 12 } : {}),
    })
    const mon = snap.monitors?.series ?? {}
    for (const [name, s] of Object.entries(mon) as [string, { x: number[]; y: number[] }][]) {
      const cur = (series[name] ??= { x: [], y: [] })
      // a fresh run restarts at x = 1: drop the old curve when the abscissa goes backwards
      if (s.x.length && cur.x.length && s.x[0] <= cur.x[cur.x.length - 1] && since === undefined) {
        cur.x = []
        cur.y = []
      }
      cur.x.push(...s.x)
      cur.y.push(...s.y)
      if (cur.x.length > MAX_POINTS) {
        cur.x = cur.x.slice(-MAX_POINTS)
        cur.y = cur.y.slice(-MAX_POINTS)
      }
    }
    const sim = snap.status?.simulation ?? {}
    const out: Snapshot = {
      session,
      state: snap.status?.state ?? '?',
      iteration: sim.iteration ?? 0,
      time: sim.time ?? 0,
      title: 'lid-driven cavity',
      series: JSON.parse(JSON.stringify(series)),
      reports: snap.reports ?? [],
      log: snap.log ?? [],
      grid: snap.grid ?? null,
      error: snap.status?.error ?? '',
    }
    await update($, snapA, () => out)
  } catch (err) {
    await update($, messageA, () => String((err as Error).message))
  } finally {
    busy = false
  }
}

async function control($: any, action: string): Promise<string> {
  if (!session) await newSession($)
  try {
    if (action === 'run') {
      for (const k of Object.keys(series)) delete series[k]
      await rpc($, 'run.start', { session })
    } else {
      await rpc($, `run.${action}`, { session })
    }
    await refresh($)
    return `${action}: ok`
  } catch (err) {
    return `${action}: ${(err as Error).message}`
  }
}

async function setSetting($: any, path: string, text: string): Promise<string> {
  const row = (await read($, rowsA)).find(r => r.path === path)
  try {
    const value = parseValue(text, row?.kind ?? 'string')
    await rpc($, 'settings.set', { session, path, value })
    await loadTree($)
    for (const k of Object.keys(series)) delete series[k]
    return `${path} = ${JSON.stringify(value)}`
  } catch (err) {
    return `${path}: ${(err as Error).message}`
  }
}

async function openPane($: any): Promise<void> {
  await $.ui.open({ id: PANE, title: 'rcfd', focus: true })
  if (!session) {
    try {
      await newSession($)
    } catch (err) {
      await update($, messageA, () => String((err as Error).message))
    }
  }
}


export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'rcfd',
      description: 'rcfd CFD panel: /rcfd [run|pause|resume|stop|new|set <path> <value>|settings|monitors]',
    })
    $.clock.every(POLL_MS, () => {
      if (session) void refresh($)
    })
    return next(e)
  })

  on('command.run', { command: 'rcfd' }, async ($, e) => {
    const [cmd, path, ...rest] = e.args.trim().split(/\s+/).filter(Boolean)
    if (!cmd || cmd === 'open') {
      await openPane($)
      return { text: 'rcfd panel opened.' }
    }
    await openPane($)
    if (cmd === 'new') {
      await newSession($)
      return { text: `new session ${session}` }
    }
    if (cmd === 'settings' || cmd === 'monitors') {
      await update($, viewA, () => cmd)
      return { text: `${cmd} view` }
    }
    if (cmd === 'set' && path) {
      const text = await setSetting($, path, rest.join(' '))
      await update($, messageA, () => text)
      return { text }
    }
    if (['run', 'pause', 'resume', 'stop', 'step'].includes(cmd)) {
      const text = await control($, cmd)
      await update($, messageA, () => text)
      return { text }
    }
    return { text: 'usage: /rcfd [open|run|pause|resume|stop|step|new|settings|monitors|set <path> <value>]' }
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const { Box, Text, Button, Input, Raster } = $.ui.resolve(e) as any
    const snap = await read($, snapA)
    const view = await read($, viewA)
    const message = await read($, messageA)
    const width = Math.max(20, (e.props?.bodyColumns ?? 60) - 2)
    const state = snap?.state ?? 'no session'
    const head = (
      <Box flexDirection="column">
        <Text bold>
          rcfd  {state}  it {snap?.iteration ?? 0}
          {snap && snap.time > 0 ? `  t=${snap.time.toPrecision(4)}` : ''}
        </Text>
        <Box>
          <Button
            key="run"
            label="Run"
            hotkey="r"
            onPress={() => {
              void control($, 'run').then(t => update($, messageA, () => t))
            }}
          />
          <Button
            key="pause"
            label="Pause"
            hotkey="p"
            onPress={() => {
              void control($, 'pause').then(t => update($, messageA, () => t))
            }}
          />
          <Button
            key="resume"
            label="Resume"
            hotkey="c"
            onPress={() => {
              void control($, 'resume').then(t => update($, messageA, () => t))
            }}
          />
          <Button
            key="stop"
            label="Stop"
            hotkey="x"
            onPress={() => {
              void control($, 'stop').then(t => update($, messageA, () => t))
            }}
          />
          <Button key="monitors" label="Monitors" hotkey="m" onPress={() => void update($, viewA, () => 'monitors')} />
          <Button key="settings" label="Settings" hotkey="s" onPress={() => void update($, viewA, () => 'settings')} />
        </Box>
        {message ? <Text dimColor>{message}</Text> : null}
        {snap?.error ? <Text color="error">{snap.error}</Text> : null}
      </Box>
    )

    if (view === 'monitors') {
      const names = Object.keys(snap?.series ?? {})
      const w = Math.max(10, width - 24)
      return (
        <Box flexDirection="column">
          {head}
          {names.length === 0 && <Text dimColor>No data yet: press Run.</Text>}
          {names.map(n => {
            const s = snap!.series[n]
            const last = s.y[s.y.length - 1]
            return (
              <Text key={n}>
                {n.padEnd(11)}
                {spark(s.y, w)} {last !== undefined ? last.toExponential(2) : ''}
              </Text>
            )
          })}
          {(snap?.reports ?? []).map(r => (
            <Text key={r.name}>
              {r.name}: {r.value !== undefined ? r.value.toPrecision(5) : r.error} {r.unit ?? ''}
            </Text>
          ))}
          {snap?.grid ? (
            <Box flexDirection="column">
              <Text dimColor>
                speed {snap.grid.min.toPrecision(3)} .. {snap.grid.max.toPrecision(3)}
              </Text>
              <Raster key="field" columns={snap.grid.cols} rows={snap.grid.rows} cells={gridCells(snap.grid)} />
            </Box>
          ) : null}
          {(snap?.log ?? []).map((l, i) => (
            <Text key={`log${i}`} dimColor wrap="truncate">
              {l}
            </Text>
          ))}
        </Box>
      )
    }

    const rows = await read($, rowsA)
    const section = await read($, sectionA)
    const selected = await read($, selectedA)
    const sections = [...new Set(rows.map(r => r.path.split('.')[0]))]
    const shown = rows.filter(r => !section || r.path.split('.')[0] === section)
    const room = Math.max(4, (e.viewport?.rows ?? 30) - 14)
    const sel = rows.find(r => r.path === selected)
    return (
      <Box flexDirection="column">
        {head}
        <Box>
          <Button key="sec:all" label={section ? 'all' : '[all]'} onPress={() => void update($, sectionA, () => '')} />
          {sections.map(s => (
            <Button key={`sec:${s}`} label={s === section ? `[${s}]` : s} onPress={() => void update($, sectionA, () => s)} />
          ))}
        </Box>
        {shown.slice(0, room).map(r => (
          <Button
            key={`row:${r.path}`}
            label={`${r.path} = ${r.value}`}
            onPress={() => void update($, selectedA, () => r.path)}
          />
        ))}
        {shown.length > room && <Text dimColor>... {shown.length - room} more: pick a section</Text>}
        {sel ? (
          <Box flexDirection="column">
            <Text dimColor>
              {sel.doc} {sel.choices ? `(${sel.choices})` : ''}
            </Text>
            <Input
              key="edit"
              label={`${sel.path} (${sel.kind})`}
              value={sel.value}
              onSubmit={(text: string) => {
                void setSetting($, sel.path, text).then(t => update($, messageA, () => t))
              }}
            />
          </Box>
        ) : (
          <Text dimColor>Select a row to edit it.</Text>
        )}
      </Box>
    )
  })
}
