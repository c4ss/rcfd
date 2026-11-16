// The Claude Code panel's one door to the main process (F-814): the only renderer code that touches window.studioClaude.
// Claude Code is not reached through the action dispatcher (src/renderer/actions, F-827): listing sessions, mirroring a
// transcript and starting `claude -p` are main-process operations behind the Claude port's allow-list
// (src/shared/claude.ts, src/main/claude/bridge.ts), not engine methods, so they have no action-registry entry (G16;
// the missing engine entry point is reported with F-814).
import type { ClaudeReply, ClaudeState, SendResult } from '../../shared/claude.js';

const bridge = () => window.studioClaude;

/** Follows the main process's Claude state; returns the function that stops following. */
export function watchClaudeState(listener: (s: ClaudeState) => void): () => void {
  const b = bridge();
  if (!b) return () => undefined;
  let live = true;
  const id = b.onState((s) => {
    if (live) listener(s);
  });
  b.state()
    .then((s) => {
      if (live) listener(s);
    })
    .catch(() => undefined);
  return () => {
    live = false;
    b.offState(id);
  };
}

const none = <T,>(): Promise<ClaudeReply<T>> =>
  Promise.resolve({ ok: false, error: { code: 'no_port', message: 'the Studio has not connected this window to Claude Code' } });

/** The panel's requests; the new state arrives on the state listener, the reply says whether it was refused and why. */
export const claudeActions = {
  select: (session: string | null): Promise<ClaudeReply<null>> => bridge()?.select(session) ?? none(),
  send: (session: string | null, text: string): Promise<ClaudeReply<SendResult>> => bridge()?.send(session, text) ?? none(),
  cancel: (): Promise<ClaudeReply<null>> => bridge()?.cancel() ?? none(),
  confirm: (token: string): Promise<ClaudeReply<null>> => bridge()?.confirm(token) ?? none(),
  refresh: (): Promise<ClaudeReply<null>> => bridge()?.refresh() ?? none(),
};
