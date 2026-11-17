// Test only (F-832): `RCFD_STUDIO_TEST_DELAY` holds a JSON object of delays in milliseconds before the engine is asked
// (`{"data": 1500, "projectOpen": 1500, "caseCheck": 1500, "caseSave": 1500}`), so the e2e tests can make an operation
// last longer than PROGRESS_AFTER_MS on any machine and cancel it while it is going. A delay ends at once when its
// operation is cancelled. Unset (always, outside the tests), every delay is 0 and nothing waits. No Electron import.

export type DelayPoint = 'data' | 'projectOpen' | 'caseCheck' | 'caseSave';

const MAX_DELAY_MS = 60_000;

/** The delays of an environment (bad or missing values are 0). */
export function parseDelays(text: string | undefined): Readonly<Record<DelayPoint, number>> {
  const out: Record<DelayPoint, number> = { data: 0, projectOpen: 0, caseCheck: 0, caseSave: 0 };
  if (!text) return out;
  try {
    const j = JSON.parse(text) as Record<string, unknown>;
    for (const k of Object.keys(out) as DelayPoint[]) {
      const v = j[k];
      if (typeof v === 'number' && Number.isFinite(v) && v > 0) out[k] = Math.min(MAX_DELAY_MS, Math.round(v));
    }
  } catch {
    // not JSON: no delays
  }
  return out;
}

const delays = parseDelays(process.env['RCFD_STUDIO_TEST_DELAY']);

/** The configured delay of one point (0 unless the tests set it). */
export const testDelayMs = (point: DelayPoint): number => delays[point];

/** Waits the configured delay of `point`; resolves early (with `false`) when `signal` aborts. */
export function testDelay(point: DelayPoint, signal?: AbortSignal): Promise<boolean> {
  const ms = delays[point];
  if (ms <= 0) return Promise.resolve(true);
  if (signal?.aborted) return Promise.resolve(false);
  return new Promise((resolve) => {
    const t = setTimeout(() => {
      signal?.removeEventListener('abort', onAbort);
      resolve(true);
    }, ms);
    const onAbort = () => {
      clearTimeout(t);
      resolve(false);
    };
    signal?.addEventListener('abort', onAbort, { once: true });
  });
}
