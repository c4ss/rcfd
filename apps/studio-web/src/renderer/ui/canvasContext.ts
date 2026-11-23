// F-890 (ADR 0787; G21): the GPU context of the page's 2D canvases, created ahead.
//
// The first time a page draws into a 2D canvas, Chromium creates the renderer's shared GPU context by a synchronous call
// to the GPU process (`GpuChannelHost::CreateViewCommandBuffer`) on the main thread, and the GPU process answers it only
// when the task its main thread is running has ended. In the Studio that first canvas was the field legend's bar
// (field/Legend.tsx), which arrives while the GPU process is making the devices and pipelines of the field view and the
// plots: the main thread then waited for the rest of one of those tasks (19 to 84 ms measured, and as long as that task
// is; ADR 0787 records the wait at over 3 s behind a task of 3.8 s).
//
// So the call is made once, here, when the page has shown its first view and is idle: the Studio has sent the GPU
// process no WebGPU work yet, and the legend's bar and every later 2D canvas find the context there. Nothing is drawn:
// the canvas has one pixel and is never added to the document.

let created = false;

/** Creates the shared GPU context of 2D canvases now, if this page has not yet (the one synchronous call). */
export function createCanvasContext(): void {
  if (created) return;
  created = true;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = 1;
    canvas.height = 1;
    // the context alone does not reach the GPU process; the first write into the canvas does
    canvas.getContext('2d')?.putImageData(new ImageData(1, 1), 0, 0);
  } catch {
    // no 2D canvas here: the first canvas that is drawn pays the call, as before
  }
}

/** Calls `createCanvasContext` at the page's next idle moment, and no later than `timeoutMs` from now. */
export function createCanvasContextWhenIdle(timeoutMs = 500): void {
  if (typeof requestIdleCallback === 'function') requestIdleCallback(() => createCanvasContext(), { timeout: timeoutMs });
  else setTimeout(createCanvasContext, 0);
}
