import { expect, test } from 'claude-code/testing'

import { flatten, gridCells, parseValue, ramp, spark } from './hooks/model'

test('sparkline falls with a falling residual', async () => {
  const s = spark([1, 0.1, 0.01, 0.001], 10)
  expect(s.length).toBe(4)
  expect(s[0]).toBe('█')
  expect(s[3]).toBe('▁')
  expect(spark([], 5)).toBe('')
})

test('settings tree flattens to rows with schema help', async () => {
  const schema = {
    properties: {
      solver: { properties: { alpha_u: { type: 'number', description: 'under-relaxation', minimum: 0.05, maximum: 1 } } },
      boundaries: { additionalProperties: { properties: { velocity: { type: 'string', enum: ['fixed', 'symmetry'] } } } },
    },
  }
  const rows: any[] = []
  flatten({ solver: { alpha_u: 0.9 }, boundaries: { lid: { velocity: 'fixed' } } }, schema, '', rows)
  expect(rows.map(r => r.path)).toEqual(['solver.alpha_u', 'boundaries.lid.velocity'])
  expect(rows[0].kind).toBe('number')
  expect(rows[0].choices).toBe('[0.05, 1]')
  expect(rows[1].choices).toBe('fixed | symmetry')
  expect(rows[1].value).toBe('"fixed"')
})

test('typed text becomes a JSON value by kind', async () => {
  expect(parseValue('0.7', 'number')).toBe(0.7)
  expect(parseValue('[1, 0, 0]', 'array')).toEqual([1, 0, 0])
  expect(parseValue('piso', 'string')).toBe('piso')
  expect(parseValue('"piso"', 'string')).toBe('piso')
  expect(() => parseValue('abc', 'number')).toThrow()
})

test('field grid becomes coloured raster cells', async () => {
  expect(ramp(0)).toBe((68 << 16) | (1 << 8) | 84)
  expect(ramp(1)).toBe((253 << 16) | (231 << 8) | 37)
  const b64 = gridCells({ cols: 2, rows: 1, values: [0, null], min: 0, max: 1 })
  // two cells x three u32 words = 24 bytes -> 32 base64 characters
  expect(b64.length).toBe(32)
})

test('pane draws run controls and the empty monitors view', async $ => {
  const ui = await $.ui.mount({
    plugin: 'rcfd-panel',
    surface: 'terminal',
    component: 'Pane',
    requestId: 'rcfd',
    props: { title: 'rcfd', isFocused: true, bodyColumns: 70, placement: 'dock' },
  } as any)
  expect(await ui.find({ type: 'Button', key: 'run' })).toBeDefined()
  expect(await ui.find({ type: 'Button', key: 'settings' })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /No data yet/ })).toBeDefined()
  await ui.press({ key: 'settings' })
  expect(await ui.find({ type: 'Text', text: /Select a row to edit/ })).toBeDefined()
  await ui.unmount()
})
