// The Studio's action layer (F-827): the registry of every UI action, its parameter schema, and the dispatcher that
// is the one path from the UI to the engine. See registry.ts and dispatcher.ts; the renderer's dispatcher is made in
// src/renderer/actions/.
export * from './params.js';
export * from './registry.js';
export * from './dispatcher.js';
