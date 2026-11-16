// The renderer's way to the service, rebuilt in Node for the action tests (F-827): the main process's ServiceClient
// attached to a service (the fake or the real `rcfd serve`), its PortBridge with the allow-list checks, Node's
// MessageChannel for Electron's, and on the far side the `call` of window.studioService as the preload script makes it.
// The dispatcher under test gets that `call`, so its requests cross the same checks as in the Studio.
import { MessageChannel, type MessagePort } from 'node:worker_threads';
import type { ActionTransport } from '../../src/shared/actions/dispatcher.js';
import { PortBridge, type PortLike } from '../../src/main/service/bridge.js';
import { ServiceClient } from '../../src/main/service/client.js';
import type { PortMessage, ServiceReply } from '../../src/shared/service.js';
import { until } from './fake-service.js';

const adapt = (p: MessagePort): PortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (l) => {
    p.on('message', l);
  },
  onClose: (l) => {
    p.on('close', l);
  },
  close: () => p.close(),
});

export interface PortTransport {
  readonly transport: ActionTransport;
  readonly client: ServiceClient;
  /** host:port of the service. */
  readonly address: string;
  close(): Promise<void>;
}

/** A transport to the service at 127.0.0.1:`port`, once connected. */
export async function portTransport(port: number): Promise<PortTransport> {
  const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port }, { backoff: () => 30, pollTimeoutMs: 2000, connectTimeoutMs: 2000 });
  client.start();
  await until(() => client.state.status === 'connected', 10_000, 'connected');
  const { port1, port2 } = new MessageChannel();
  const bridge = new PortBridge(adapt(port1), client);
  let nextId = 1;
  const pending = new Map<number, (r: ServiceReply) => void>();
  port2.on('message', (m: PortMessage) => {
    if (m.type !== 'reply') return;
    const done = pending.get(m.id);
    pending.delete(m.id);
    done?.(m.reply);
  });
  const transport: ActionTransport = {
    call: (method, params, options) =>
      new Promise((resolve) => {
        const id = nextId++;
        pending.set(id, resolve);
        port2.postMessage({
          op: 'call',
          id,
          method,
          ...(params !== undefined ? { params } : {}),
          ...(options?.timeoutMs !== undefined ? { timeoutMs: options.timeoutMs } : {}),
        });
      }),
  };
  return {
    transport,
    client,
    address: `127.0.0.1:${port}`,
    close: async () => {
      bridge.dispose();
      port2.close();
      await client.shutdown();
    },
  };
}
