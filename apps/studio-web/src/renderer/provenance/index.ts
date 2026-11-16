// F-828: the provenance inspector. Surfaces wrap a number in <Provenance subject={...}> with a source of ./sources.
export { Provenance, triggerName } from './Provenance.js';
export type { ProvenanceProps, ProvenanceSubject } from './Provenance.js';
export { cellValue, countValue, counterValue, reportValue, seriesValue, statusValue } from './sources.js';
export type { StopFacts } from './sources.js';
