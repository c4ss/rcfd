// The case editor's one door to the main process (F-812): the only renderer code that touches window.studioCase.
// The case files are not reached through the action dispatcher (src/renderer/actions, F-827): opening, saving and making
// cases combine the engine's project-server tools with service methods in the main process (src/shared/case.ts), and the
// registry holds one service method per action. The session actions of the editor (session.create, session.close and the
// settings) do go through the dispatcher.
import type { CaseBridge, CaseReply } from '../../shared/case.js';

const missing: CaseReply<never> = { ok: false, error: { code: 'no_port', message: 'the Studio has no case editor in this window' } };

const bridge = (): CaseBridge | undefined => window.studioCase;

/** The case port's operations; every one resolves with a reply, never throws. */
export const caseDoor: CaseBridge = {
  state: () => bridge()?.state() ?? Promise.resolve(missing),
  sources: () => bridge()?.sources() ?? Promise.resolve(missing),
  prepare: (path) => bridge()?.prepare(path) ?? Promise.resolve(missing),
  attach: (session, path, hash) => bridge()?.attach(session, path, hash) ?? Promise.resolve(missing),
  status: (session) => bridge()?.status(session) ?? Promise.resolve(missing),
  conflict: (session) => bridge()?.conflict(session) ?? Promise.resolve(missing),
  save: (session, onto, options) => bridge()?.save(session, onto, options) ?? Promise.resolve(missing),
  create: (name, source) => bridge()?.create(name, source) ?? Promise.resolve(missing),
  duplicate: (path, name) => bridge()?.duplicate(path, name) ?? Promise.resolve(missing),
  check: (path, options) => bridge()?.check(path, options) ?? Promise.resolve(missing),
  forget: (session) => bridge()?.forget(session) ?? Promise.resolve(missing),
};
