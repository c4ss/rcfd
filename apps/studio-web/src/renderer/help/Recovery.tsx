// The way out of an engine error (F-830, G19): the error catalogue's recovery line for its code, as the engine wrote it
// (`help.errors`). Renders nothing when the catalogue has no line for the code or is not available.
import { recoveryOf, useErrorCatalogue } from './engine.js';

export function EngineRecovery({ code, className }: { code: unknown; className?: string }) {
  const line = recoveryOf(useErrorCatalogue(), code);
  if (line === null) return null;
  return (
    <p className={className ?? 'eng-recovery'} data-words="exception" data-recovery-code={String(code)}>
      {line}
    </p>
  );
}
