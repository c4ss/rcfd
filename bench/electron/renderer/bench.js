// @ts-check
'use strict';
// Renderer of the Electron performance spike (ADR 0350, F-447): takes the checks of ADR 0350 section 3 as amendment 1
// (A1.4) fixes them and reports the numbers to the main process. Measurement only: no framework, one canvas.

/** @type {MessagePort} */
let port;
/** @type {Map<number, {res: (v: any) => void, rej: (e: Error) => void}>} */
const pending = new Map();
let nextId = 1;

/**
 * @param {string} op
 * @param {object} [args]
 * @returns {Promise<any>}
 */
function call(op, args) {
  return new Promise((res, rej) => {
    const id = nextId++;
    pending.set(id, { res, rej });
    port.postMessage({ id, op, args });
  });
}

const now = () => performance.now();
const raf = () => new Promise((r) => requestAnimationFrame(r));
const sleep = (/** @type {number} */ ms) => new Promise((r) => setTimeout(r, ms));
const statusEl = /** @type {HTMLElement} */ (document.getElementById('status'));
const say = (/** @type {string} */ t) => {
  statusEl.textContent = t;
  console.log(t);
};

/** @param {number[]} a */
function stats(a) {
  const s = Float64Array.from(a).sort();
  const n = s.length;
  const rank = (/** @type {number} */ q) => s[Math.min(n, Math.max(1, Math.ceil(q * n))) - 1];
  let sum = 0;
  for (const x of s) sum += x;
  return { n, p50: rank(0.5), p99: rank(0.99), min: s[0], max: s[n - 1], mean: sum / n };
}

// ---------------------------------------------------------------- GPU

/** @type {GPUDevice} */
let device;
/** @type {GPUCanvasContext} */
let ctx;
/** @type {GPUTextureFormat} */
let format;
/** @type {GPUTextureView} */
let depthView;
/** @type {Record<string, unknown>} */
let adapterInfo = {};

async function gpuSetup() {
  if (!navigator.gpu) throw new Error('navigator.gpu is missing (no WebGPU in this window)');
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) throw new Error('no WebGPU adapter');
  const i = adapter.info;
  adapterInfo = { vendor: i.vendor, architecture: i.architecture, device: i.device, description: i.description };
  device = await adapter.requestDevice({
    requiredLimits: {
      maxStorageBufferBindingSize: adapter.limits.maxStorageBufferBindingSize,
      maxBufferSize: adapter.limits.maxBufferSize,
    },
  });
  device.addEventListener('uncapturederror', (e) => console.log(`GPU error: ${/** @type {GPUUncapturedErrorEvent} */ (e).error.message}`));
  const canvas = /** @type {HTMLCanvasElement} */ (document.getElementById('view'));
  ctx = /** @type {GPUCanvasContext} */ (canvas.getContext('webgpu'));
  format = navigator.gpu.getPreferredCanvasFormat();
  ctx.configure({ device, format, alphaMode: 'opaque' });
  depthView = device
    .createTexture({ size: [canvas.width, canvas.height], format: 'depth24plus', usage: GPUTextureUsage.RENDER_ATTACHMENT })
    .createView();
}

// Five anchor colours of the viridis map at 0, 1/4, 1/2, 3/4 and 1 (quoted from memory of the published table, joined
// linearly). The spike measures drawing speed; matching rcfd-render's colours value for value is phase 2 (ADR 0350 §5).
const VIRIDIS = `
fn viridis(t: f32) -> vec3f {
  let c = array<vec3f, 5>(vec3f(0.267, 0.005, 0.329), vec3f(0.229, 0.322, 0.546), vec3f(0.128, 0.567, 0.551),
                          vec3f(0.369, 0.789, 0.383), vec3f(0.993, 0.906, 0.144));
  let x = clamp(t, 0.0, 1.0) * 4.0;
  let i = min(u32(x), 3u);
  return mix(c[i], c[i + 1u], x - f32(i));
}`;

const SURFACE_WGSL = `
struct U { mvp: mat4x4f, range: vec2f, pad: vec2f }
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> field: array<f32>;
struct VO { @builtin(position) pos: vec4f, @location(0) t: f32 }
@vertex fn vs(@location(0) p: vec3f, @location(1) cell: u32) -> VO {
  var o: VO;
  o.pos = u.mvp * vec4f(p, 1.0);
  o.t = (field[cell] - u.range.x) / max(u.range.y - u.range.x, 1e-30);
  return o;
}
${VIRIDIS}
@fragment fn fs(i: VO) -> @location(0) vec4f { return vec4f(viridis(i.t), 1.0); }`;

const SOUP_WGSL = `
struct U { mvp: mat4x4f, range: vec2f, pad: vec2f }
@group(0) @binding(0) var<uniform> u: U;
struct VO { @builtin(position) pos: vec4f, @location(0) t: f32 }
@vertex fn vs(@location(0) p: vec3f, @location(1) s: f32) -> VO {
  var o: VO;
  o.pos = u.mvp * vec4f(p, 1.0);
  o.t = (s - u.range.x) / max(u.range.y - u.range.x, 1e-30);
  return o;
}
${VIRIDIS}
@fragment fn fs(i: VO) -> @location(0) vec4f { return vec4f(viridis(i.t), 1.0); }`;

/**
 * @param {string} code
 * @param {GPUVertexFormat} second format of the second vertex attribute
 */
function pipelineOf(code, second) {
  const module = device.createShaderModule({ code });
  return device.createRenderPipeline({
    layout: 'auto',
    vertex: {
      module,
      entryPoint: 'vs',
      buffers: [
        {
          arrayStride: 16,
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x3' },
            { shaderLocation: 1, offset: 12, format: second },
          ],
        },
      ],
    },
    fragment: { module, entryPoint: 'fs', targets: [{ format }] },
    primitive: { topology: 'triangle-list', cullMode: 'none' },
    depthStencil: { format: 'depth24plus', depthWriteEnabled: true, depthCompare: 'less' },
  });
}

/**
 * Column-major view-projection of a camera on a circle about `centre` (clip z in 0..1).
 * @param {number} angle
 * @param {[number, number, number]} centre
 * @param {number} radius
 * @param {number} height
 */
function orbitMatrix(angle, centre, radius, height) {
  const eye = [centre[0] + radius * Math.cos(angle), centre[1] + radius * Math.sin(angle), centre[2] + height];
  const f = [centre[0] - eye[0], centre[1] - eye[1], centre[2] - eye[2]];
  const fl = Math.hypot(f[0], f[1], f[2]);
  f[0] /= fl;
  f[1] /= fl;
  f[2] /= fl;
  const up = [0, 0, 1];
  const s = [f[1] * up[2] - f[2] * up[1], f[2] * up[0] - f[0] * up[2], f[0] * up[1] - f[1] * up[0]];
  const sl = Math.hypot(s[0], s[1], s[2]);
  s[0] /= sl;
  s[1] /= sl;
  s[2] /= sl;
  const v = [s[1] * f[2] - s[2] * f[1], s[2] * f[0] - s[0] * f[2], s[0] * f[1] - s[1] * f[0]];
  const view = [
    s[0], v[0], -f[0], 0,
    s[1], v[1], -f[1], 0,
    s[2], v[2], -f[2], 0,
    -(s[0] * eye[0] + s[1] * eye[1] + s[2] * eye[2]), -(v[0] * eye[0] + v[1] * eye[1] + v[2] * eye[2]), f[0] * eye[0] + f[1] * eye[1] + f[2] * eye[2], 1,
  ];
  const fovy = Math.PI / 4;
  const aspect = 1280 / 800;
  const near = 0.05;
  const far = 50;
  const t = 1 / Math.tan(fovy / 2);
  const proj = [t / aspect, 0, 0, 0, 0, t, 0, 0, 0, 0, far / (near - far), -1, 0, 0, (near * far) / (near - far), 0];
  const out = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      let x = 0;
      for (let k = 0; k < 4; k++) x += proj[k * 4 + r] * view[c * 4 + k];
      out[c * 4 + r] = x;
    }
  }
  return out;
}

/**
 * @param {GPURenderPipeline} pipeline
 * @param {GPUBindGroup} group
 * @param {GPUBuffer} vertices
 * @param {number} count
 */
function draw(pipeline, group, vertices, count) {
  const enc = device.createCommandEncoder();
  const pass = enc.beginRenderPass({
    colorAttachments: [
      { view: ctx.getCurrentTexture().createView(), clearValue: { r: 0.06, g: 0.08, b: 0.1, a: 1 }, loadOp: 'clear', storeOp: 'store' },
    ],
    depthStencilAttachment: { view: depthView, depthClearValue: 1, depthLoadOp: 'clear', depthStoreOp: 'store' },
  });
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, group);
  pass.setVertexBuffer(0, vertices);
  pass.draw(count);
  pass.end();
  device.queue.submit([enc.finish()]);
}

/**
 * The surface of a store, ready to draw coloured by a cell field held in a storage buffer.
 * @param {{points: ArrayBuffer, triangles: ArrayBuffer, cells: ArrayBuffer}} s
 * @param {number} nCells
 */
function surfaceState(s, nCells) {
  const points = new Float32Array(s.points);
  const tri = new Uint32Array(s.triangles);
  const cells = new Uint32Array(s.cells);
  const n = tri.length;
  const raw = new ArrayBuffer(16 * n);
  const f = new Float32Array(raw);
  const u = new Uint32Array(raw);
  for (let i = 0; i < n; i++) {
    const p = 3 * tri[i];
    f[4 * i] = points[p];
    f[4 * i + 1] = points[p + 1];
    f[4 * i + 2] = points[p + 2];
    u[4 * i + 3] = cells[(i / 3) | 0];
  }
  const vertices = device.createBuffer({ size: raw.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(vertices, 0, raw);
  const field = device.createBuffer({ size: 4 * nCells, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  const uniform = device.createBuffer({ size: 80, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const pipeline = pipelineOf(SURFACE_WGSL, 'uint32');
  const group = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: uniform } },
      { binding: 1, resource: { buffer: field } },
    ],
  });
  let angle = 0.7;
  return {
    count: n,
    /** @param {ArrayBuffer} values the cell field */
    show(values) {
      const v = new Float32Array(values);
      let lo = Infinity;
      let hi = -Infinity;
      for (let i = 0; i < v.length; i++) {
        const x = v[i];
        if (x < lo) lo = x;
        if (x > hi) hi = x;
      }
      device.queue.writeBuffer(field, 0, values);
      const ub = new Float32Array(20);
      ub.set(orbitMatrix(angle, [0.5, 0.5, 0.5], 2.2, 1.1), 0);
      ub[16] = lo;
      ub[17] = hi;
      device.queue.writeBuffer(uniform, 0, ub);
      angle += 0.01;
      draw(pipeline, group, vertices, n);
    },
    destroy() {
      vertices.destroy();
      field.destroy();
      uniform.destroy();
    },
  };
}

// ---------------------------------------------------------------- checks

// The registered quantities (ADR 0350 amendment 1, A1.4). `--smoke` swaps in small ones to try the code on the 1e5 box;
// numbers of a smoke run are not results.
const FULL = {
  mid: '1e6', midCells: 1000000, midBox: [100, 100, 100], big: '4e6', p2Events: 300, p3Steps: 100000, p3Repeats: 20,
  p4Warm: 20, p4Timed: 200, r0Repeats: 20, p5Passes: 10, p6Passes: 5, p7Repeats: 5, warmFrames: 120, frames: 1200,
  p12: /** @type {[string, string[]][]} */ ([['1e5', ['p', 'ux', 'uy', 'uz']], ['1e6', ['p', 'ux', 'uy', 'uz']], ['4e6', ['p']]]),
};
const SMOKE = {
  mid: '1e5', midCells: 100000, midBox: [50, 50, 40], big: '1e5', p2Events: 8, p3Steps: 300, p3Repeats: 3,
  p4Warm: 2, p4Timed: 5, r0Repeats: 2, p5Passes: 1, p6Passes: 1, p7Repeats: 2, warmFrames: 10, frames: 60,
  p12: /** @type {[string, string[]][]} */ ([['1e5', ['p', 'ux']]]),
};
let Q = FULL;

const store = (/** @type {string} */ size, /** @type {string} */ codec) => `project/runs/box-${size}-${codec}/fields`;
const fieldOf = (/** @type {string} */ route, /** @type {string} */ path, /** @type {number} */ snapshot, /** @type {string} */ field) =>
  call(`${route}.field`, { path, snapshot, field });

/** @type {Record<string, () => Promise<any>>} */
const checks = {
  async p1() {
    const r = await call('p1');
    return {
      'service.info': stats(r.ms['service.info']),
      'run.status': stats(r.ms['run.status']),
      protocol: r.info.protocol,
      protocol_version: r.info.protocol_version,
      data: r.info.data,
      methods: r.info.methods.length,
    };
  },

  async p2() {
    const sync = async () => {
      let best = { rtt: Infinity, offset: 0 };
      for (let i = 0; i < 200; i++) {
        const t0 = performance.timeOrigin + now();
        const r = await call('rpc', { method: 'data.info' });
        const t1 = performance.timeOrigin + now();
        if (t1 - t0 < best.rtt) best = { rtt: t1 - t0, offset: r.unix_ns / 1e6 - (t0 + t1) / 2 };
      }
      return best;
    };
    const before = await sync();
    const session = await call('session', { n: [50, 50, 40] });
    let after = (await call('poll', { after: 0, timeout_ms: 0, limit: 1 })).next;
    // skip what is already in the log
    for (;;) {
      const r = await call('poll', { after, timeout_ms: 0, limit: 1000 });
      if (r.events.length === 0) break;
      after = r.next;
    }
    await call('rpc', { method: 'run.start', params: { session, max_steps: 100000 } });
    /** @type {Map<number, number>} */
    const received = new Map();
    const t0 = now();
    while (received.size < Q.p2Events && now() - t0 < 120000) {
      const r = await call('poll', { after, timeout_ms: 1000, session });
      const t = performance.timeOrigin + now();
      for (const e of r.events) if (e.kind === 'iteration') received.set(e.seq, t);
      after = r.next;
    }
    const elapsed = now() - t0;
    await call('rpc', { method: 'run.stop', params: { session } });
    const afterSync = await sync();
    const stamps = await call('stamps');
    /** @type {Map<number, number>} */
    const at = new Map();
    for (let i = 0; i < stamps.seq.length; i++) at.set(stamps.seq[i], stamps.ms[i]);
    const lat = [];
    let missing = 0;
    for (const [seq, t] of received) {
      const s = at.get(seq);
      if (s === undefined) missing++;
      else lat.push(t + before.offset - s);
    }
    return {
      latency_ms: lat.length ? stats(lat) : null,
      events: received.size,
      missing_stamps: missing,
      seconds: elapsed / 1000,
      clock: { before, after: afterSync, drift_ms: afterSync.offset - before.offset },
    };
  },

  async p3() {
    const session = await call('session', { n: [4, 4, 4], report: true });
    const st = await call('rpc', { method: 'run.step', params: { session, count: Q.p3Steps } });
    const all = await call('rpc', { method: 'monitors.get', params: { session } });
    const names = Object.keys(all.series).sort().slice(0, 4);
    const samples = names.map((n) => all.series[n].x.length);
    const ms = [];
    let bytes = 0;
    let parsed = 0;
    for (let i = 0; i < Q.p3Repeats; i++) {
      const t = now();
      const line = await call('rpcRaw', { method: 'monitors.get', params: { session, names } });
      const doc = JSON.parse(line);
      ms.push(now() - t);
      bytes = line.length;
      parsed = Object.keys(doc.result.series).length;
    }
    return { ms: stats(ms), names, samples, series_parsed: parsed, reply_chars: bytes, state: st.state, iteration: st.iteration };
  },

  /**
   * F-456 (ADR 0353) I1: an increment of 100 samples of each of four series that hold 1e5. Own session with the same
   * set-up as p3; `since` is the abscissa of the 101st sample from the end; 20 warm-up requests, then 200 timed, request to
   * parsed object. I2s: `run.step {count: 100}` once afterwards, wall time, no limit.
   */
  async p3i() {
    const session = await call('session', { n: [4, 4, 4], report: true });
    const st = await call('rpc', { method: 'run.step', params: { session, count: Q.p3Steps } });
    const all = await call('rpc', { method: 'monitors.get', params: { session } });
    const names = Object.keys(all.series).sort().slice(0, 4);
    const samples = names.map((n) => all.series[n].x.length);
    const xs = all.series[names[0]].x;
    const since = xs[xs.length - 101];
    const warm = Q === FULL ? 20 : 3;
    const timed = Q === FULL ? 200 : 10;
    const ms = [];
    let counts = [];
    let chars = 0;
    for (let i = 0; i < warm + timed; i++) {
      const t = now();
      const line = await call('rpcRaw', { method: 'monitors.get', params: { session, names, since } });
      const doc = JSON.parse(line);
      const dt = now() - t;
      if (i >= warm) ms.push(dt);
      counts = names.map((n) => doc.result.series[n].x.length);
      chars = line.length;
    }
    const t1 = now();
    await call('rpc', { method: 'run.step', params: { session, count: 100 } });
    const step100_ms = now() - t1;
    return { ms: stats(ms), names, samples, since, counts_returned: counts, reply_chars: chars, step100_ms, state: st.state };
  },

  async p4() {
    /** @type {Record<string, any>} */
    const out = {};
    const buf = device.createBuffer({ size: 4 * Q.midCells, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    for (const route of ['r1', 'r2']) {
      for (const codec of ['raw', 'deflate']) {
        say(`P4 ${route} ${codec}`);
        const path = store(Q.mid, codec);
        if (route === 'r2') await call('r2.open', { path });
        const ms = [];
        for (let i = 0; i < Q.p4Warm + Q.p4Timed; i++) {
          const t = now();
          const values = await fieldOf(route, path, i % 20, 'p');
          device.queue.writeBuffer(buf, 0, values);
          await device.queue.onSubmittedWorkDone();
          if (i >= Q.p4Warm) ms.push(now() - t);
        }
        out[`${route}-${codec}`] = stats(ms);
      }
    }
    say('P4 r0');
    const t0 = now();
    const session = await call('session', { n: Q.midBox });
    await call('rpc', { method: 'run.step', params: { session, count: 1 } });
    const setup = now() - t0;
    const ms = [];
    let chars = 0;
    for (let i = 0; i < Q.r0Repeats; i++) {
      const t = now();
      const line = await call('rpcRaw', { method: 'field.slice', params: { session, name: 'p', max_points: Q.midCells } });
      const values = new Float32Array(JSON.parse(line).result.values);
      device.queue.writeBuffer(buf, 0, values);
      await device.queue.onSubmittedWorkDone();
      ms.push(now() - t);
      chars = line.length;
      if (values.length !== Q.midCells) throw new Error(`R0 returned ${values.length} values`);
    }
    out.r0 = stats(ms);
    out.r0_reply_chars = chars;
    out.r0_session_setup_ms = setup;
    await call('rpc', { method: 'session.close', params: { session } });
    buf.destroy();
    return out;
  },

  p5: () => scrub(Q.mid, Q.p5Passes),
  p6: () => scrub(Q.big, Q.p6Passes),

  async p7() {
    /** @type {Record<string, any>} */
    const out = {};
    const path = store(Q.mid, 'raw');
    for (const route of ['r1', 'r2']) {
      const ms = [];
      for (let rep = 0; rep < Q.p7Repeats; rep++) {
        await sleep(200);
        const t = now();
        const open = route === 'r1' ? await call('rpc', { method: 'data.open', params: { path } }) : await call('r2.open', { path, fresh: true });
        const s = await call(`${route}.surface`, { path });
        const values = await fieldOf(route, path, 19, 'p');
        const state = surfaceState(s, open.n_cells);
        state.show(values);
        await device.queue.onSubmittedWorkDone();
        await raf();
        ms.push(now() - t);
        state.destroy();
      }
      out[route] = { ...stats(ms), first: ms[0], all: ms };
    }
    return out;
  },

  async p8() {
    say('P8 building the soup');
    const nx = 1000;
    const ny = 500;
    const raw = new Float32Array(4 * 6 * nx * ny);
    const z = (/** @type {number} */ x, /** @type {number} */ y) => 0.25 * Math.sin(3 * Math.PI * x) * Math.cos(3 * Math.PI * y);
    let k = 0;
    const put = (/** @type {number} */ i, /** @type {number} */ j) => {
      const x = i / nx;
      const y = j / ny;
      const h = z(x, y);
      raw[k++] = x;
      raw[k++] = y;
      raw[k++] = h;
      raw[k++] = h;
    };
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        put(i, j);
        put(i + 1, j);
        put(i + 1, j + 1);
        put(i, j);
        put(i + 1, j + 1);
        put(i, j + 1);
      }
    }
    const count = 6 * nx * ny;
    const vertices = device.createBuffer({ size: raw.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
    device.queue.writeBuffer(vertices, 0, raw);
    const uniform = device.createBuffer({ size: 80, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const pipeline = pipelineOf(SOUP_WGSL, 'float32');
    const group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: uniform } }] });
    await device.queue.onSubmittedWorkDone();
    // the display's rate as this window sees it: 240 empty frames
    const empty = [];
    let last = /** @type {number} */ (await raf());
    for (let i = 0; i < 240; i++) {
      const t = /** @type {number} */ (await raf());
      empty.push(t - last);
      last = t;
    }
    const ub = new Float32Array(20);
    ub[16] = -0.25;
    ub[17] = 0.25;
    const frames = [];
    const callbacks = [];
    let prev = 0;
    let prevCb = 0;
    say(`P8 drawing ${count / 3} triangles${config.uncapped ? ' (no frame-rate limit)' : ''}`);
    for (let f = 0; f < Q.warmFrames + Q.frames + 1; f++) {
      const stamp = /** @type {number} */ (await raf());
      const cb = now();
      ub.set(orbitMatrix((2 * Math.PI * f) / 600, [0.5, 0.5, 0], 1.6, 0.9), 0);
      device.queue.writeBuffer(uniform, 0, ub);
      draw(pipeline, group, vertices, count);
      if (config.uncapped) await device.queue.onSubmittedWorkDone();
      if (f > Q.warmFrames) {
        frames.push(stamp - prev);
        callbacks.push(cb - prevCb);
      }
      prev = stamp;
      prevCb = cb;
    }
    await device.queue.onSubmittedWorkDone();
    vertices.destroy();
    const total = frames.reduce((a, b) => a + b, 0);
    return {
      triangles: count / 3,
      uncapped: config.uncapped,
      frame_ms: stats(frames),
      callback_interval_ms: stats(callbacks),
      fps: (1000 * frames.length) / total,
      empty_frame_ms: stats(empty),
      display_hz: 1000 / stats(empty).p50,
    };
  },

  async p9() {
    const mem = async () => ({ renderer: await /** @type {any} */ (window).host.memory(), processes: await call('mem') });
    await sleep(1000);
    const before = await mem();
    const path = store(Q.mid, 'raw');
    const held = [];
    for (let i = 0; i < 20; i++) {
      const values = new Float32Array(await fieldOf('r1', path, i, 'p'));
      const buf = device.createBuffer({ size: values.byteLength, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
      device.queue.writeBuffer(buf, 0, values);
      held.push({ values, buf });
    }
    await device.queue.onSubmittedWorkDone();
    await sleep(2000);
    const after = await mem();
    let sum = 0;
    for (const h of held) sum += h.values[12345];
    return {
      raw_bytes: 20 * 4 * Q.midCells,
      limit_kb: (2.5 * 20 * 4 * Q.midCells + 300e6) / 1024,
      before,
      after,
      renderer_private_kb: after.renderer.private,
      held: held.length,
      checksum: sum,
    };
  },

  async p11() {
    const entries = await call('transcript');
    const list = /** @type {HTMLElement} */ (document.getElementById('list'));
    const spacer = /** @type {HTMLElement} */ (document.getElementById('spacer'));
    const rowH = 56;
    list.style.display = 'block';
    spacer.style.height = `${entries.length * rowH}px`;
    const viewH = list.clientHeight;
    /** @type {HTMLElement[]} */
    const pool = [];
    let maxRows = 0;
    const render = () => {
      const first = Math.max(0, Math.floor(list.scrollTop / rowH) - 3);
      const lastRow = Math.min(entries.length - 1, Math.ceil((list.scrollTop + viewH) / rowH) + 3);
      const n = lastRow - first + 1;
      while (pool.length < n) {
        const row = document.createElement('div');
        row.className = 'row';
        row.append(document.createElement('span'), document.createTextNode(''));
        /** @type {HTMLElement} */ (row.firstChild).className = 'role';
        list.appendChild(row);
        pool.push(row);
      }
      for (let i = 0; i < pool.length; i++) {
        const row = pool[i];
        if (i >= n) {
          row.style.display = 'none';
          continue;
        }
        const e = entries[first + i];
        row.style.display = '';
        row.style.top = `${(first + i) * rowH}px`;
        /** @type {HTMLElement} */ (row.firstChild).textContent = e.role;
        /** @type {Text} */ (row.lastChild).data = e.text;
      }
      maxRows = Math.max(maxRows, n);
    };
    render();
    let prev = /** @type {number} */ (await raf());
    for (let i = 0; i < Q.warmFrames; i++) prev = /** @type {number} */ (await raf());
    const end = entries.length * rowH - viewH;
    const frames = [];
    for (let f = 1; f <= Q.frames; f++) {
      list.scrollTop = (end * f) / Q.frames;
      render();
      const t = /** @type {number} */ (await raf());
      frames.push(t - prev);
      prev = t;
    }
    const reached = list.scrollTop;
    list.style.display = 'none';
    return { entries: entries.length, frame_ms: stats(frames), rows_in_document: pool.length, max_rows_shown: maxRows, scrolled_px: reached, end_px: end };
  },

  async p12() {
    /** @type {Record<string, any>} */
    const out = { differing_bits: 0, values_compared: 0, fields_compared: 0, length_mismatches: 0, by: {} };
    /** @param {ArrayBuffer} a @param {ArrayBuffer} b */
    const diffBits = (a, b) => {
      if (a.byteLength !== b.byteLength) {
        out.length_mismatches++;
        return 8 * Math.abs(a.byteLength - b.byteLength) + 1;
      }
      const x = new Uint32Array(a);
      const y = new Uint32Array(b);
      let bits = 0;
      for (let i = 0; i < x.length; i++) {
        let d = x[i] ^ y[i];
        if (d !== 0) {
          d = d - ((d >>> 1) & 0x55555555);
          d = (d & 0x33333333) + ((d >>> 2) & 0x33333333);
          bits += (((d + (d >>> 4)) & 0x0f0f0f0f) * 0x01010101) >>> 24;
        }
      }
      return bits;
    };
    for (const [size, fields] of Q.p12) {
      for (const codec of ['raw', 'deflate']) {
        const path = store(size, codec);
        await call('r2.open', { path });
        for (const route of ['r1', 'r2']) {
          say(`P12 ${size} ${codec} ${route}`);
          let bits = 0;
          let n = 0;
          for (let snapshot = 0; snapshot < 20; snapshot++) {
            for (const field of fields) {
              const got = await fieldOf(route, path, snapshot, field);
              const want = await call('ref', { size, snapshot, field });
              bits += diffBits(got, want);
              n += got.byteLength / 4;
              out.fields_compared++;
            }
          }
          out.by[`${size}-${codec}-${route}`] = { differing_bits: bits, values: n };
          out.differing_bits += bits;
          out.values_compared += n;
        }
      }
    }
    // the surface of the 1e6 case by both routes (not a registered check; reported)
    const a = await call('r1.surface', { path: store(Q.mid, 'raw') });
    const b = await call('r2.surface', { path: store(Q.mid, 'raw') });
    out.surface_r1_vs_r2_differing_bits = diffBits(a.points, b.points) + diffBits(a.triangles, b.triangles) + diffBits(a.cells, b.cells);
    // NC2: block 3 (values 196608 .. 262143) of one received field with its bytes rotated by one
    const got = new Uint8Array(await fieldOf('r1', store(Q.mid, 'raw'), 0, 'p'));
    const want = await call('ref', { size: Q.mid, snapshot: 0, field: 'p' });
    const clean = diffBits(got.buffer, want);
    const lo = 4 * 3 * 65536;
    const hi = 4 * 4 * 65536;
    const firstByte = got[lo];
    got.copyWithin(lo, lo + 1, hi);
    got[hi - 1] = firstByte;
    out.nc2 = { differing_bits_before_shift: clean, differing_bits_after_shift: diffBits(got.buffer, want) };
    return out;
  },
};

/**
 * P5 and P6: passes over the 20 snapshots, one per frame request, both routes.
 * @param {string} size
 * @param {number} passes
 */
async function scrub(size, passes) {
  /** @type {Record<string, any>} */
  const out = {};
  const path = store(size, 'raw');
  for (const route of ['r1', 'r2']) {
    say(`scrubbing ${size} ${route}`);
    const open = route === 'r1' ? await call('rpc', { method: 'data.open', params: { path } }) : await call('r2.open', { path });
    const state = surfaceState(await call(`${route}.surface`, { path }), open.n_cells);
    const starts = [];
    for (let pass = -1; pass < passes; pass++) {
      for (let k = 0; k < 20; k++) {
        const t = now();
        if (pass >= 0) starts.push(t);
        state.show(await fieldOf(route, path, k, 'p'));
        await raf();
      }
    }
    const end = now();
    const frames = [];
    for (let i = 0; i < starts.length; i++) frames.push((i + 1 < starts.length ? starts[i + 1] : end) - starts[i]);
    out[route] = {
      snapshots: starts.length,
      per_second: (1000 * starts.length) / (end - starts[0]),
      frame_ms: stats(frames),
      triangles: state.count / 3,
    };
    await device.queue.onSubmittedWorkDone();
    state.destroy();
  }
  return out;
}

/** @type {{checks: string[], uncapped: boolean, startOnly: boolean, smoke: boolean, electron: string, chrome: string, node: string}} */
let config;

async function projectView() {
  const runs = await call('runs');
  const ul = /** @type {HTMLElement} */ (document.getElementById('runs'));
  ul.style.display = 'block';
  for (const r of runs) {
    const li = document.createElement('li');
    li.textContent = `${r.id}  (${(r.storeBytes / 1e6).toFixed(1)} MB)`;
    ul.appendChild(li);
  }
  statusEl.textContent = `project: ${runs.length} runs`;
  await raf();
  return runs.length;
}

async function main() {
  config = await call('config');
  if (config.smoke) Q = SMOKE;
  if (config.startOnly) {
    const runs = await projectView();
    await call('done', { readyUnixMs: Date.now(), runs });
    return;
  }
  /** @type {Record<string, any>} */
  const results = { config, startedUnixMs: Date.now(), checks: {} };
  let gpuError = '';
  try {
    await gpuSetup();
    results.adapter = adapterInfo;
    results.canvasFormat = format;
  } catch (e) {
    gpuError = String(e);
    results.gpuError = gpuError;
  }
  results.window = { innerWidth, innerHeight, devicePixelRatio, visibility: document.visibilityState };
  const needsGpu = new Set(['p4', 'p5', 'p6', 'p7', 'p8', 'p9']);
  for (const name of config.checks) {
    const t = now();
    try {
      if (!checks[name]) throw new Error('no such check');
      if (gpuError && needsGpu.has(name)) throw new Error(`not run: ${gpuError}`);
      say(`running ${name}`);
      results.checks[name] = await checks[name]();
    } catch (e) {
      results.checks[name] = { error: String(e && /** @type {Error} */ (e).stack ? /** @type {Error} */ (e).stack : e) };
      console.log(`${name} failed: ${e}`);
    }
    results.checks[name].wall_s = (now() - t) / 1000;
  }
  say('done');
  await call('done', results);
}

window.addEventListener('message', (e) => {
  if (e.data !== 'port' || port) return;
  port = e.ports[0];
  port.onmessage = (m) => {
    const p = pending.get(m.data.id);
    if (!p) return;
    pending.delete(m.data.id);
    if (m.data.ok) p.res(m.data.value);
    else p.rej(new Error(m.data.error));
  };
  main().catch((err) => call('done', { error: String(err && err.stack ? err.stack : err) }));
});
