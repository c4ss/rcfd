// Preload script of the Studio window. F-800 skeleton: it exposes nothing.
//
// F-801 fills it with the allow-list of IPC calls of docs/studio-guardrails.md G10, exposed to the page through
// contextBridge. It is CommonJS (.cts, emitted as .cjs) because a sandboxed preload cannot be an ES module.
export {};
