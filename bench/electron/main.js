// @ts-check
'use strict';
// Main process of the Electron performance spike (ADR 0350, F-447). All I/O happens here (ADR 0350 amendment 1,
// A1.3): the two TCP connections to the rcfd service, file reads and inflate. The renderer is sandboxed and
// context-isolated and reaches this process over one MessagePort.
//
//   electron . --data=<folder of f447_bench gen> --server=<f447_bench.exe> --out=<results.json> --checks=p1,p4,...
//   electron . --start --data=<folder> --out=<file>     (P10: the project view only, then quit)
//   --uncapped: no frame-rate limit and no vertical sync (the second run of P8)

const { app, BrowserWindow, Menu, MessageChannelMain, screen } = require('electron');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { RpcClient, DataClient, r1Field, r1Surface } = require('./io/rpc.js');
const { Store } = require('./io/tsstore.js');

/** @param {string} name */
function arg(name) {
  const hit = process.argv.find((a) => a.startsWith(`--${name}=`));
  return hit ? hit.slice(name.length + 3) : '';
}
const flag = (/** @type {string} */ name) => process.argv.includes(`--${name}`);

const dataDir = arg('data');
const outFile = arg('out');
const startOnly = flag('start');
const uncapped = flag('uncapped');

// A window covered by another one must keep drawing: the measurement is of frames, not of the desktop's stacking.
app.commandLine.appendSwitch('disable-features', 'CalculateNativeWinOcclusion');
if (uncapped) {
  app.commandLine.appendSwitch('disable-frame-rate-limit');
  app.commandLine.appendSwitch('disable-gpu-vsync');
}

const REPORT = `[reports.lid_drag]
type = "force"
patch = "ymax"
component = "viscous"
`;

/**
 * @param {[number, number, number]} n
 * @param {boolean} [report] add a force report (a fourth monitor series beside the three residuals, for P3)
 */
function boxToml(n, report) {
  const walls = ['xmin', 'xmax', 'ymin', 'zmin', 'zmax'].map((p) => `[boundaries.${p}]\nvelocity = "fixed"\n`).join('');
  return `title = "F-447 lid-driven box"
[mesh]
generator = "box_3d"
nx = ${n[0]}
ny = ${n[1]}
nz = ${n[2]}
lx = 1.0
ly = 1.0
lz = 1.0
[physics]
nu = 0.01
${walls}[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 1000000
tolerance_u = 1e-14
tolerance_p = 1e-14
${report ? REPORT : ''}`;
}

/** @type {import('node:child_process').ChildProcess | null} */
let server = null;
/** @type {{rpc: number, data: number, events: string, root: string} | null} */
let ports = null;
/** @type {RpcClient | null} */
let rpc = null;
/** @type {RpcClient | null} */
let rpcEvents = null;
/** @type {DataClient | null} */
let data = null;
/** @type {Map<string, Store>} */
const stores = new Map();

function startServer() {
  return new Promise((resolve, reject) => {
    const child = spawn(arg('server'), ['serve', dataDir], { stdio: ['pipe', 'pipe', 'inherit'] });
    server = child;
    let text = '';
    child.stdout.on('data', (d) => {
      text += d.toString('utf8');
      const nl = text.indexOf('\n');
      if (nl >= 0 && !ports) {
        ports = JSON.parse(text.slice(0, nl));
        resolve(undefined);
      }
    });
    child.on('error', reject);
    child.on('exit', (c) => {
      if (!ports) reject(new Error(`the server ended with code ${c}`));
    });
  });
}

async function connect() {
  await startServer();
  const p = /** @type {{rpc: number, data: number}} */ (ports);
  rpc = new RpcClient(p.rpc);
  rpcEvents = new RpcClient(p.rpc);
  data = new DataClient(p.data);
  await Promise.all([rpc.connect(), rpcEvents.connect(), data.connect()]);
}

/** @param {string} rel */
function store(rel) {
  let s = stores.get(rel);
  if (!s) {
    s = new Store(path.join(dataDir, rel));
    stores.set(rel, s);
  }
  return s;
}

/**
 * @param {[number, number, number]} n
 * @param {boolean} [report]
 */
async function session(n, report) {
  const c = /** @type {RpcClient} */ (rpc);
  const s = (await c.call('session.create', { toml: boxToml(n, report) })).session;
  await c.call('run.initialize', { session: s });
  return s;
}

function cpuTimes() {
  let busy = 0;
  let total = 0;
  for (const c of os.cpus()) {
    const t = c.times;
    busy += t.user + t.sys + t.nice + t.irq;
    total += t.user + t.sys + t.nice + t.irq + t.idle;
  }
  return { busy, total };
}
const cpuStart = cpuTimes();

/** @type {Record<string, (a: any) => Promise<any> | any>} */
const ops = {
  config: () => ({
    checks: arg('checks').split(',').filter(Boolean),
    uncapped,
    startOnly,
    smoke: flag('smoke'),
    displays: screen.getAllDisplays().map((d) => ({
      primary: d.id === screen.getPrimaryDisplay().id,
      size: d.size,
      scaleFactor: d.scaleFactor,
      hz: d.displayFrequency,
    })),
    windowBounds: BrowserWindow.getAllWindows().map((w) => w.getContentBounds()),
    electron: process.versions.electron,
    chrome: process.versions.chrome,
    node: process.versions.node,
  }),
  rpc: (a) => /** @type {RpcClient} */ (rpc).call(a.method, a.params),
  rpcRaw: (a) => /** @type {RpcClient} */ (rpc).raw(a.method, a.params),
  poll: (a) => /** @type {RpcClient} */ (rpcEvents).call('events.poll', a),
  session: (a) => session(a.n, a.report),
  // P1: 50 warm-up calls, then 1000 timed calls of each method on one connection
  p1: async () => {
    const c = /** @type {RpcClient} */ (rpc);
    const s = await session([50, 50, 40]);
    /** @type {Record<string, number[]>} */
    const out = {};
    for (const [method, params] of /** @type {[string, object][]} */ ([
      ['service.info', {}],
      ['run.status', { session: s }],
    ])) {
      for (let i = 0; i < 50; i++) await c.call(method, params);
      const ms = [];
      for (let i = 0; i < (flag('smoke') ? 20 : 1000); i++) {
        const t = performance.now();
        await c.call(method, params);
        ms.push(performance.now() - t);
      }
      out[method] = ms;
    }
    return { session: s, ms: out, info: await c.call('service.info', {}) };
  },
  'r1.field': (a) => r1Field(/** @type {RpcClient} */ (rpc), /** @type {DataClient} */ (data), a.path, a.snapshot, a.field),
  'r1.surface': (a) => r1Surface(/** @type {RpcClient} */ (rpc), /** @type {DataClient} */ (data), a.path),
  'r2.open': (a) => {
    if (a.fresh) {
      const old = stores.get(a.path);
      if (old) old.close();
      stores.delete(a.path);
    }
    const s = store(a.path);
    return { n_cells: s.nCells, n_points: s.nPoints, snapshots: s.snaps.map((x) => ({ step: x.step, time: x.time })) };
  },
  'r2.field': (a) => store(a.path).field(a.snapshot, a.field),
  'r2.surface': (a) => store(a.path).surface(),
  ref: (a) => {
    const b = fs.readFileSync(path.join(dataDir, 'ref', a.size, `${a.snapshot}-${a.field}.f32`));
    return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
  },
  // the server's stamp of every event: seq -> milliseconds since the Unix epoch
  stamps: () => {
    const text = fs.readFileSync(/** @type {{events: string}} */ (ports).events, 'utf8');
    const seq = [];
    const ms = [];
    for (const line of text.split('\n')) {
      const p = line.split(' ');
      if (p.length !== 3) continue;
      seq.push(Number(p[0]));
      ms.push(Number(BigInt(p[2]) / 1000n) / 1000);
    }
    return { seq, ms };
  },
  mem: () =>
    app.getAppMetrics().map((m) => ({
      type: m.type,
      pid: m.pid,
      workingSetKB: m.memory.workingSetSize,
      privateKB: m.memory.privateBytes,
    })),
  runs: () => {
    const dir = path.join(dataDir, 'project', 'runs');
    return fs
      .readdirSync(dir, { withFileTypes: true })
      .filter((d) => d.isDirectory())
      .map((d) => {
        const f = path.join(dir, d.name, 'fields', 'store.rts');
        return { id: d.name, storeBytes: fs.existsSync(f) ? fs.statSync(f).size : 0 };
      });
  },
  // P11: 1e4 transcript lines in the Claude Code transcript form, written once, then read from the file
  transcript: () => {
    const file = path.join(dataDir, 'transcript-1e4.jsonl');
    if (!fs.existsSync(file)) {
      let seed = 12345;
      const next = () => (seed = (Math.imul(seed, 1103515245) + 12345) >>> 0) / 4294967296;
      const words = 'the solver mesh residual converged pressure velocity fan blade case run field store step time'.split(' ');
      const lines = [];
      for (let i = 0; i < 10000; i++) {
        const len = 40 + Math.floor(next() * 1160);
        let text = '';
        while (text.length < len) text += words[Math.floor(next() * words.length)] + ' ';
        const role = i % 2 === 0 ? 'user' : 'assistant';
        lines.push(
          JSON.stringify({
            type: role,
            uuid: `f447-${i}`,
            parentUuid: i === 0 ? null : `f447-${i - 1}`,
            sessionId: 'f447',
            timestamp: new Date(1762905600000 + 1000 * i).toISOString(),
            message: { role, content: role === 'user' ? text.slice(0, len) : [{ type: 'text', text: text.slice(0, len) }] },
          }),
        );
      }
      fs.writeFileSync(file, lines.join('\n') + '\n');
    }
    return fs
      .readFileSync(file, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((l) => {
        const e = JSON.parse(l);
        const c = e.message.content;
        return { role: e.message.role, text: typeof c === 'string' ? c : c.map((/** @type {{text: string}} */ x) => x.text).join('') };
      });
  },
  log: (a) => {
    console.log(a);
  },
  done: (a) => {
    const cpuEnd = cpuTimes();
    a.cpuBusyFractionDuringRun = (cpuEnd.busy - cpuStart.busy) / Math.max(1, cpuEnd.total - cpuStart.total);
    a.finishedUnixMs = Date.now();
    if (outFile) fs.writeFileSync(outFile, JSON.stringify(a, null, 1));
    setTimeout(() => app.quit(), 50);
  },
};

app.on('will-quit', () => {
  for (const c of [rpc, rpcEvents, data]) if (c) c.close();
  if (server && server.stdin) server.stdin.end();
});

app.whenReady().then(async () => {
  if (!startOnly) await connect();
  Menu.setApplicationMenu(null);
  const win = new BrowserWindow({
    width: 1280,
    height: 800,
    useContentSize: true,
    show: false,
    backgroundColor: '#101418',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: false,
    },
  });
  win.webContents.on('console-message', (e) => console.log(`[renderer] ${e.message}`));
  win.webContents.on('render-process-gone', (_e, d) => {
    console.error(`renderer gone: ${d.reason}`);
    if (outFile) fs.writeFileSync(outFile, JSON.stringify({ error: `renderer gone: ${d.reason}` }));
    app.exit(3);
  });
  win.webContents.once('did-finish-load', () => {
    const { port1, port2 } = new MessageChannelMain();
    port1.on('message', async (e) => {
      const { id, op, args } = e.data;
      try {
        const f = ops[op];
        if (!f) throw new Error(`unknown op ${op}`);
        port1.postMessage({ id, ok: true, value: await f(args ?? {}) });
      } catch (err) {
        port1.postMessage({ id, ok: false, error: String(err && /** @type {Error} */ (err).stack ? /** @type {Error} */ (err).stack : err) });
      }
    });
    port1.start();
    win.webContents.postMessage('port', null, [port2]);
  });
  await win.loadFile(path.join(__dirname, 'renderer', 'index.html'));
  win.showInactive();
});

app.on('window-all-closed', () => app.quit());
