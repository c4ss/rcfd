// The renderer's one door to the engine (F-827, G16). Surfaces call actions of the registry (src/shared/actions/)
// through the dispatcher made here; this module is the only renderer code that touches window.studioService, and
// tests/unit/actions-static.spec.ts fails when any other file under src/renderer does.
//
//   import { dispatch, availability } from '../actions';
//   const r = await dispatch('run.start', { session });   // r.ok ? r.result : r.error (code, message): never throws
//   const a = availability('run.pause', { session }, { session: { id: session, state } });  // a.available, a.reason
//
// Besides actions it passes on the two streams that are not calls: the connection state and the service's events
// (the main process owns events.poll; ADR 0801 addendum 2).
import type { ServiceEventBatch, ServiceReply, ServiceState } from '../../shared/service.js';
import { Dispatcher, type ActionReply, type DispatchOptions } from '../../shared/actions/dispatcher.js';
import type { ActionContext, ActionId, ActionParams, Availability } from '../../shared/actions/registry.js';

let dispatcher: Dispatcher | null = null;
let state: ServiceState | null = null;

/** The renderer's dispatcher, made on first use (the action log lives as long as the page). */
export function actions(): Dispatcher {
  if (!dispatcher) {
    const bridge = window.studioService;
    dispatcher = new Dispatcher(bridge, { context: () => ({ service: state }) });
    bridge.onState((s) => {
      state = s;
    });
    void bridge.state().then((s) => {
      state ??= s;
    });
  }
  return dispatcher;
}

/** Dispatches one action: parameters checked, availability checked, the engine call made and logged. */
export function dispatch<Id extends ActionId>(id: Id, params: ActionParams<Id>, options?: DispatchOptions): Promise<ActionReply> {
  return actions().dispatch(id, params, options);
}

/** Whether an action is available now (for a control's disabled state and its tooltip, G19), and why not. */
export function availability<Id extends ActionId>(id: Id, params: Partial<ActionParams<Id>> = {}, context: ActionContext = {}): Availability {
  return actions().availability(id, params as Record<string, unknown>, context);
}

/** The connection state now, and every later change; returns the function that stops listening. */
export function watchServiceState(listener: (s: ServiceState) => void): () => void {
  actions();
  const bridge = window.studioService;
  const id = bridge.onState(listener);
  void bridge.state().then(listener);
  return () => bridge.offState(id);
}

/** The service's events from now on (optionally of one session); resolves with the function that stops them. */
export async function watchEvents(listener: (b: ServiceEventBatch) => void, session?: string): Promise<ServiceReply<() => void>> {
  const bridge = window.studioService;
  const r = await bridge.subscribe(listener, session !== undefined ? { session } : {});
  if (!r.ok) return r;
  const sub = r.result;
  return { ok: true, result: () => void bridge.unsubscribe(sub) };
}
