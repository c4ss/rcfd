// F-817b (G9, power): three.js's WebGPURenderer drawn on demand.
//
// three's renderer keeps a frame loop of its own (three/src/renderers/common/Animation.js): the first `render()` or
// `compute()` starts it, and it then asks for an animation frame for ever, even when nothing is drawn again. Each tick
// only does three's per-frame bookkeeping (`info.reset()`, `nodeFrame.update()`, the user loop of `setAnimationLoop`).
// The field canvas and every plot each had one, so the run workspace asked for frames at the display's rate at rest.
//
// `drawOnDemand` gives that loop an animation context which lets it ask for `idleFrames` frames after the last
// `render()` or `compute()`, then stops it: with the default of 2, two or three frames after the last draw (three when the
// loop had already asked for its next frame before that draw). Two, not one, so that a draw every frame (an orbit drag)
// keeps the loop running exactly as before rather than stopping and starting it each frame. The next draw after a stop
// starts the loop the way three starts it the first time (the bookkeeping of a frame, then the draw), so every draw sees
// the per-frame state it saw before (two draws in one frame share three's frame id, draws in different frames do not):
// what is drawn does not change. A user loop set with `setAnimationLoop` keeps the loop running as before.

/** The parts of three's renderer and its private animation loop used here (three does not type the loop). */
interface Loop {
  setContext(context: AnimationContext): void;
}

interface AnimationContext {
  requestAnimationFrame(callback: FrameRequestCallback): number | null;
  cancelAnimationFrame(id: number | null): void;
}

interface Drawable {
  render(...args: never[]): unknown;
  compute(...args: never[]): unknown;
  getAnimationLoop(): unknown;
}

/** Stops `renderer`'s own frame loop once it has asked for `idleFrames` frames since the last draw; false when three has no such loop. */
export function drawOnDemand(renderer: Drawable, idleFrames = 2): boolean {
  const loop = (renderer as unknown as { _animation?: Loop | null })._animation;
  const host = typeof self === 'undefined' ? null : self;
  if (!loop || typeof loop.setContext !== 'function' || !host) return false;
  // frames the loop has asked for since the last draw
  let idle = 0;
  loop.setContext({
    requestAnimationFrame: (callback) => (renderer.getAnimationLoop() !== null || idle++ < idleFrames ? host.requestAnimationFrame(callback) : null),
    cancelAnimationFrame: (id) => {
      if (id !== null) host.cancelAnimationFrame(id);
    },
  });
  for (const name of ['render', 'compute'] as const) {
    const draw = renderer[name] as (...args: unknown[]) => unknown;
    (renderer as unknown as Record<string, unknown>)[name] = function (this: unknown, ...args: unknown[]) {
      idle = 0; // before three's own start: it asks for the next frame first
      return draw.apply(this, args);
    };
  }
  return true;
}
