export type Series = { x: number[]; y: number[] }

export type Row = { path: string; value: string; kind: string; doc: string; choices: string }

export type Grid = { cols: number; rows: number; values: (number | null)[]; min: number; max: number }

export type Snapshot = {
  session: string
  state: string
  iteration: number
  time: number
  title: string
  series: Record<string, Series>
  reports: { name: string; value?: number; unit?: string; error?: string }[]
  log: string[]
  grid: Grid | null
  error: string
}

declare module 'claude-code' {
  interface PluginState {
    'rcfd-panel': {
      snap: Snapshot | null
      rows: Row[]
      view: 'monitors' | 'settings'
      section: string
      selected: string
      message: string
    }
  }
}
