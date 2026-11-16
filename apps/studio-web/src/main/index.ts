// Main process of rcfd Studio. F-800 skeleton: one window that shows the renderer's empty project view.
//
// F-801 turns this into the hardened shell of docs/studio-guardrails.md G10 (explicit sandbox, context isolation and
// nodeIntegration settings asserted by a test, a strict Content Security Policy, no remote content, an allow-list of
// IPC calls). Service and file I/O will live in this process (src/main), never in the renderer.
import { app, BrowserWindow } from 'electron';
import { fileURLToPath } from 'node:url';

const beside = (rel: string): string => fileURLToPath(new URL(rel, import.meta.url));

function openWindow(): void {
  const win = new BrowserWindow({
    width: 1280,
    height: 800,
    show: false,
    webPreferences: { preload: beside('../preload/index.cjs') },
  });
  win.once('ready-to-show', () => win.show());
  void win.loadFile(beside('../renderer/index.html'));
}

void app.whenReady().then(openWindow);
app.on('window-all-closed', () => app.quit());
