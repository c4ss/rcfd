// The responsiveness budget of the Studio (F-832, guardrail G21) and the cancel hook the doors carry. Shared by all three
// processes, so it uses neither Node nor DOM types.
//
// The numbers were registered in apps/studio-web/README.md ("Responsiveness and cancellable work") and ADR 0801,
// addendum 9 (F-832), before the measurement was written; tests/e2e/responsiveness.spec.ts imports them from here.
// Changing one needs the maintainer, like a limit of G9.

/** Longest task allowed on the renderer's main thread while a run is opened, fields load and the scrubber moves (ms). */
export const LONG_TASK_LIMIT_MS = 100;

/** Longest animation frame allowed in the same window (ms): G9's P5 "no frame > 150 ms". */
export const LONG_FRAME_LIMIT_MS = 150;

/** An operation still going after this long shows its progress and a Cancel; a shorter one shows nothing (ms). */
export const PROGRESS_AFTER_MS = 300;

/**
 * How a cancel crosses a door that cannot carry an AbortSignal (contextBridge copies objects; a signal would arrive
 * dead): the caller passes `onCancel`, the preload script calls it once with the function that cancels this request,
 * and the caller calls that function when its signal aborts. Functions cross contextBridge as proxies.
 */
export interface CancelOptions {
  readonly onCancel?: (cancel: () => void) => void;
}

/** The operations of F-832 that show progress and can be cancelled (G21). */
export const WORK_KINDS = ['project.open', 'run.open', 'field.load', 'case.check', 'case.save', 'mesh.check'] as const;
export type WorkKind = (typeof WORK_KINDS)[number];
