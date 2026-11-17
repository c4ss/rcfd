// The data port of each window (F-804), the way src/main/index.ts hands out the service port (F-803): after each load of
// an app page (dom-ready, URL on app://studio/) the page's preload script gets one MessagePort on DATA_PORT_CHANNEL; the
// old one is closed. The port stays in the preload script; the page only sees `window.studioData`. No ipcMain channel
// is used (the shell's allow-list stays app.info and app.ready).
import { MessageChannelMain, type MessagePortMain, type WebContents } from 'electron';
import { DATA_PORT_CHANNEL } from '../../shared/data.js';
import { isAppUrl } from '../security.js';
import { DataPortBridge, type DataBackend, type DataPortLike } from './bridge.js';

const adapt = (p: MessagePortMain): DataPortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (listener) => {
    p.on('message', (e) => listener(e.data));
  },
  onClose: (listener) => {
    p.on('close', listener);
  },
  close: () => p.close(),
});

export class DataPorts {
  private readonly bridges = new Map<number, DataPortBridge>();

  constructor(private readonly backend: DataBackend) {}

  /** After a load of one of our own pages: a new port for its preload script, and the old one closed. */
  offer(contents: WebContents): void {
    this.drop(contents.id);
    if (contents.isDestroyed() || !isAppUrl(contents.getURL())) return;
    const { port1, port2 } = new MessageChannelMain();
    this.bridges.set(contents.id, new DataPortBridge(adapt(port1), this.backend));
    port1.start();
    contents.postMessage(DATA_PORT_CHANNEL, null, [port2]);
  }

  drop(id: number): void {
    this.bridges.get(id)?.dispose();
    this.bridges.delete(id);
  }
}
