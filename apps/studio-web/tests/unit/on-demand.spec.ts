// Unit tests of F-817b's on-demand drawing (src/renderer/ui/onDemand.ts) against three.js's own frame loop
// (three/src/renderers/common/Animation.js, the class WebGPURenderer uses), driven by a frame clock of the test:
// - without it, one draw leaves three's loop asking for a frame every frame for ever (what the workspace did at rest);
// - with it, the loop runs two or three frames after the last draw and stops; the next draw starts it again;
// - the per-frame bookkeeping each draw sees is the same as before: two draws in one frame share three's frame id, two
//   draws in different frames do not (so what is drawn does not change);
// - a user loop set with setAnimationLoop keeps running.
import { expect, test } from '@playwright/test';
import { drawOnDemand } from '../../src/renderer/ui/onDemand.js';

interface AnimationLoop {
  start(): void;
  stop(): void;
  setAnimationLoop(callback: ((time: number) => void) | null): void;
  getAnimationLoop(): unknown;
}
type AnimationClass = new (renderer: unknown, nodes: unknown, info: unknown) => AnimationLoop;

// three ships no types for this module; the path is a variable so the compiler does not look for them
const ANIMATION = 'three/src/renderers/common/Animation.js';
let Animation: AnimationClass;
test.beforeAll(async () => {
  Animation = ((await import(ANIMATION)) as { default: AnimationClass }).default;
});

/** A frame clock standing in for the page's requestAnimationFrame: `frame()` runs the callbacks asked for so far. */
class Clock {
  private next = 1;
  private queue = new Map<number, FrameRequestCallback>();
  asked = 0;
  ran = 0;
  now = 0;
  requestAnimationFrame = (cb: FrameRequestCallback): number => {
    this.asked++;
    const id = this.next++;
    this.queue.set(id, cb);
    return id;
  };
  cancelAnimationFrame = (id: number): void => {
    this.queue.delete(id);
  };
  frame(): void {
    this.now += 1000 / 60;
    const due = [...this.queue.values()];
    this.queue.clear();
    for (const cb of due) {
      this.ran++;
      cb(this.now);
    }
  }
  frames(n: number): void {
    for (let i = 0; i < n; i++) this.frame();
  }
}

/** The parts of WebGPURenderer three's loop touches, with three's real Animation; `render()` starts it as three does. */
function fakeRenderer() {
  const nodes = { nodeFrame: { frameId: 0, update() { this.frameId++; } } };
  const info = { autoReset: true, frame: 0, reset() {} };
  const r = {
    _inspector: { isRunning: false, begin() {}, finish() {} },
    _animation: null as AnimationLoop | null,
    /** three's frame id seen by each draw. */
    drawn: [] as number[],
    render() {
      this._animation!.start();
      this.drawn.push(nodes.nodeFrame.frameId);
    },
    compute() {
      this._animation!.start();
    },
    getAnimationLoop() {
      return this._animation!.getAnimationLoop();
    },
  };
  r._animation = new Animation(r, nodes, info);
  return r;
}

let clock: Clock;
test.beforeEach(() => {
  clock = new Clock();
  (globalThis as unknown as { self: unknown }).self = clock;
});
test.afterEach(() => {
  delete (globalThis as unknown as { self?: unknown }).self;
});

test('three’s own loop asks for a frame every frame after one draw (the loop F-817b stops)', () => {
  const r = fakeRenderer();
  r.render();
  clock.frames(100);
  expect(clock.ran).toBe(100);
});

test('on demand: at most three frames after the last draw, then none; the next draw starts the loop again', () => {
  const r = fakeRenderer();
  expect(drawOnDemand(r)).toBe(true);
  // a draw at rest: three asks for the next frame; two loop frames, then the loop stops
  r.render();
  clock.frames(100);
  expect(clock.ran).toBe(2);
  // a second draw in the same frame, after the loop asked for its next frame: one frame more
  r.render();
  r.render();
  clock.frames(100);
  expect(clock.ran).toBe(5);
  // drawn every frame while the user interacts: the loop keeps running, one loop frame per frame, as before
  let asked = clock.asked;
  for (let i = 0; i < 50; i++) {
    r.render();
    clock.frame();
  }
  expect(clock.ran).toBe(55);
  expect(clock.asked - asked).toBe(51); // one to start the stopped loop, then one per frame
  clock.frames(100);
  expect(clock.ran).toBe(57);
  asked = clock.asked;
  clock.frames(100);
  expect(clock.asked).toBe(asked);
  r.compute();
  clock.frames(10);
  expect(clock.ran).toBe(59);
});

test('on demand, each draw sees the frame bookkeeping it saw before: one frame id per frame, a new one per frame', () => {
  // draws at these frames, several in one frame at times, with long rests between
  const script = [0, 0, 1, 2, 2, 2, 50, 51, 51, 300, 301, 302, 302, 900];
  const run = (onDemand: boolean): boolean[] => {
    clock = new Clock();
    (globalThis as unknown as { self: unknown }).self = clock;
    const r = fakeRenderer();
    if (onDemand) drawOnDemand(r);
    let at = 0;
    for (const f of script) {
      while (at < f) {
        clock.frame();
        at++;
      }
      r.render();
    }
    // whether each draw saw a new frame id against the draw before it
    return r.drawn.slice(1).map((id, i) => id !== r.drawn[i]);
  };
  const before = run(false);
  const after = run(true);
  expect(after).toEqual(before);
  expect(before).toEqual(script.slice(1).map((f, i) => f !== script[i]));
});

test('a user loop set with setAnimationLoop keeps running on demand', () => {
  const r = fakeRenderer();
  drawOnDemand(r);
  let ticks = 0;
  r._animation!.setAnimationLoop(() => ticks++);
  r._animation!.start(); // three runs the first tick at once
  clock.frames(30);
  expect(ticks).toBe(31);
  r._animation!.setAnimationLoop(null);
  clock.frames(30);
  expect(ticks).toBe(31);
  expect(clock.ran).toBe(33); // and the loop stops three frames later
});

test('without three’s loop (a renderer that has none) nothing is changed', () => {
  const plain = { render() {}, compute() {}, getAnimationLoop: () => null };
  const render = plain.render;
  expect(drawOnDemand(plain)).toBe(false);
  expect(plain.render).toBe(render);
});
