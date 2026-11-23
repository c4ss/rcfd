// F-890 (ADR 0787, amendment 3): holding the GPU process's main thread for a known time, to test that nothing on the
// page's main thread waits for the GPU process meanwhile.
//
// The first use of a 2D canvas in a page makes Chromium create the renderer's shared GPU context by a synchronous call
// to the GPU process, and the call is answered only when the task the GPU process's main thread is running has ended.
// A test cannot choose what the Studio's own WebGPU work is doing at that moment, so this makes the state itself: a
// WebGPU device of the test's own, and a compute pipeline whose shader is long enough that the GPU process takes at
// least `minMs` to make it (a chain of arithmetic statements with a random constant, so no shader cache holds it; the
// length is found by doubling). `startGpuHold` starts one such pipeline; an observer records when the element matching
// `selector` enters the document, so the test can check that it did so while the hold was running.
import type { Page } from '@playwright/test';

export interface GpuHold {
  /** Statements of the calibrated shader, and the time the GPU process took for it in the calibration (ms). */
  readonly statements: number;
  readonly calibratedMs: number;
  /** performance.now() when the hold was started; null before. */
  readonly startedAt: number | null;
  /** How long the GPU process took for the hold's pipeline (ms); null until it is done. */
  readonly heldMs: number | null;
  /** performance.now() when the element matching the selector was first seen in the document; null while it is not. */
  readonly seenAt: number | null;
}

interface HoldState {
  statements: number;
  calibratedMs: number;
  startedAt: number | null;
  heldMs: number | null;
  seenAt: number | null;
  start(): void;
}

/** Calibrates the shader (at least `minMs`) and starts watching for `selector`; resolves once the GPU process is idle again. */
export async function armGpuHold(page: Page, selector: string, minMs: number): Promise<{ statements: number; calibratedMs: number }> {
  return page.evaluate(
    async ([sel, min]) => {
      const gpu = (navigator as unknown as { gpu?: { requestAdapter(): Promise<{ requestDevice(): Promise<unknown> } | null> } }).gpu;
      const adapter = await gpu?.requestAdapter();
      if (!adapter) throw new Error('no WebGPU adapter: the GPU process cannot be held');
      const device = (await adapter.requestDevice()) as {
        createShaderModule(d: { code: string }): unknown;
        createComputePipeline(d: unknown): unknown;
        queue: { submit(b: unknown[]): void; onSubmittedWorkDone(): Promise<void> };
      };
      const shader = (n: number): string => {
        const seed = (Math.random() * 1000).toFixed(6);
        let s = '@group(0) @binding(0) var<storage, read_write> o: array<f32>;\n@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n  var x: f32 = f32(id.x) + ' + seed + ';\n';
        for (let i = 0; i < n; i++) s += `  x = fract(sin(x * ${(1.1 + (i % 7)).toFixed(1)} + ${seed}) * 43758.5453 + f32(${i}u));\n`;
        return s + '  o[id.x] = x;\n}\n';
      };
      /** One pipeline of `n` statements; resolves with the time until the GPU process has made it. */
      const make = async (n: number): Promise<number> => {
        const t = performance.now();
        const module = device.createShaderModule({ code: shader(n) });
        device.createComputePipeline({ layout: 'auto', compute: { module, entryPoint: 'main' } });
        device.queue.submit([]);
        await device.queue.onSubmittedWorkDone();
        return performance.now() - t;
      };
      let n = 500;
      let ms = await make(n);
      while (ms < min && n < 512_000) {
        n *= 2;
        ms = await make(n);
      }
      if (ms < min) throw new Error(`the GPU process made a shader of ${n} statements in ${ms.toFixed(0)} ms: it cannot be held for ${min} ms`);
      const state: HoldState = {
        statements: n,
        calibratedMs: ms,
        startedAt: null,
        heldMs: null,
        seenAt: null,
        start() {
          state.startedAt = performance.now();
          void make(n).then((held) => (state.heldMs = held));
        },
      };
      (window as unknown as { __gpuHold: HoldState }).__gpuHold = state;
      const observer = new MutationObserver(() => {
        if (document.querySelector(sel!) === null) return;
        observer.disconnect();
        state.seenAt = performance.now();
      });
      observer.observe(document.documentElement, { childList: true, subtree: true });
      return { statements: n, calibratedMs: ms };
    },
    [selector, minMs] as const,
  );
}

/**
 * Starts the hold and, in the same task, clicks the element with this role and accessible name (a DOM click: a click
 * through the window would wait for frames the held GPU process does not deliver).
 */
export const startGpuHoldAndClick = (page: Page, role: string, name: string): Promise<void> =>
  page.evaluate(
    ([r, n]) => {
      const el = [...document.querySelectorAll<HTMLElement>(`[role="${r}"]`)].find((e) => (e.getAttribute('aria-label') ?? e.textContent ?? '').trim() === n);
      if (!el) throw new Error(`no ${r} named ${n}`);
      (window as unknown as { __gpuHold: HoldState }).__gpuHold.start();
      el.click();
    },
    [role, name] as const,
  );

/** The hold's state now. */
export const gpuHold = (page: Page): Promise<GpuHold> =>
  page.evaluate(() => {
    const s = (window as unknown as { __gpuHold: HoldState }).__gpuHold;
    return { statements: s.statements, calibratedMs: s.calibratedMs, startedAt: s.startedAt, heldMs: s.heldMs, seenAt: s.seenAt };
  });
