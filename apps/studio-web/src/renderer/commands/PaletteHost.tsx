// The command palette's key listener (F-831; G20, G8), mounted once beside every route (main.tsx). It binds the global
// shortcuts of the one table (src/shared/commands/shortcuts.ts) on the window, in the capture phase (React Aria's fields
// stop the propagation of the keys they handle), and loads the palette itself on first use, so the project home's cold
// start (P10) does not carry it.
import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { globalCommandFor } from '../../shared/commands/shortcuts.js';
import { onCommandRequest } from './host.js';

const CommandPalette = lazy(() => import('./CommandPalette.js'));

/** A request to the palette: a command id, numbered so the same command twice is two requests. */
export interface PaletteRequest {
  readonly id: string;
  readonly n: number;
}

export function PaletteHost() {
  const [request, setRequest] = useState<PaletteRequest | null>(null);
  const n = useRef(0);
  useEffect(() => {
    const ask = (id: string) => {
      n.current += 1;
      setRequest({ id, n: n.current });
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat || e.isComposing) return;
      const id = globalCommandFor(e);
      if (id === null) return;
      e.preventDefault();
      e.stopPropagation();
      ask(id);
    };
    window.addEventListener('keydown', onKey, true);
    const off = onCommandRequest(ask);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      off();
    };
  }, []);
  if (request === null) return null;
  return (
    <Suspense fallback={null}>
      <CommandPalette request={request} />
    </Suspense>
  );
}
