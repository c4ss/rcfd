// The Claude port of each window (F-814), the way src/main/data/port.ts hands out the data port: after each load of an
// app page (dom-ready, URL on app://studio/) the page's preload script gets one MessagePort on CLAUDE_PORT_CHANNEL; the
// old one is closed. The port stays in the preload script; the page only sees `window.studioClaude`. No ipcMain channel
// is used (the shell's allow-list stays app.info and app.ready).
import { MessageChannelMain, type MessagePortMain, type WebContents } from 'electron';
import { CLAUDE_PORT_CHANNEL } from '../../shared/claude.js';
import { isAppUrl } from '../security.js';
import { ClaudePortBridge, type BridgePanel, type ClaudePortLike } from './bridge.js';

const adapt = (p: MessagePortMain): ClaudePortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (listener) => {
    p.on('message', (e) => listener(e.data));
  },
  onClose: (listener) => {
    p.on('close', listener);
  },
  close: () => p.close(),
});

export class ClaudePorts {
  private readonly bridges = new Map<number, ClaudePortBridge>();

  constructor(private readonly panel: BridgePanel) {}

  offer(contents: WebContents): void {
    this.drop(contents.id);
    if (contents.isDestroyed() || !isAppUrl(contents.getURL())) return;
    const { port1, port2 } = new MessageChannelMain();
    this.bridges.set(contents.id, new ClaudePortBridge(adapt(port1), this.panel));
    port1.start();
    contents.postMessage(CLAUDE_PORT_CHANNEL, null, [port2]);
  }

  drop(id: number): void {
    this.bridges.get(id)?.dispose();
    this.bridges.delete(id);
  }
}
