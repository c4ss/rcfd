// Small pieces the residual and monitor plots share (F-811): the no-series sentence with its pictogram, a legend swatch
// that draws the line's own dash pattern, and number formatting.
import { StatePictogram, type RunState } from '../ui/index.js';
import type { LineStyle } from './style.js';

/** A run without a series of this kind: the run's pictogram and the reason in words (G6, G8). */
export function NoSeries({ picto, sentence, height }: { picto: RunState; sentence: string; height?: number }) {
  return (
    <p className="plot-empty" role="status" style={height !== undefined ? { minHeight: height } : undefined}>
      <StatePictogram state={picto} />
      <span>{sentence}</span>
    </p>
  );
}

/** The swatch of a line: the same colour role, width and dashes as the drawn line. Decorative (the name is beside it). */
export function Swatch({ style }: { style: LineStyle }) {
  return (
    <svg className={`plot-swatch plot-role--${style.role}`} width="26" height="6" viewBox="0 0 26 6" aria-hidden="true" focusable="false">
      <path d="M0 3H26" strokeWidth={style.width} strokeDasharray={style.dash.length === 2 ? `${style.dash[0]} ${style.dash[1]}` : undefined} />
    </svg>
  );
}

/** A solver value as text: 4 significant figures, exponent form outside 1e-3 .. 1e5; "not finite" for NaN and infinities. */
export function fmt(v: number): string {
  if (!Number.isFinite(v)) return 'not finite';
  if (v === 0) return '0';
  const a = Math.abs(v);
  return a >= 1e-3 && a < 1e5 ? String(Number(v.toPrecision(4))) : v.toExponential(3).replace('e+', 'e');
}

/** A residual as text: 3 significant figures in exponent form. */
export function fmtResidual(v: number): string {
  if (!Number.isFinite(v)) return 'not finite';
  return v.toExponential(2).replace('e+', 'e');
}

/** The abscissa's caption from the reply's x_label. */
export function xCaptionOf(xLabel: string | null): string {
  if (xLabel === 'time') return 'time, s';
  return xLabel || 'abscissa';
}

/** Where on the abscissa a value stands: "iteration 523", "t = 1.25 s". */
export function atAbscissa(xLabel: string | null, x: number): string {
  if (xLabel === 'time') return `t = ${fmt(x)} s`;
  return `${xLabel || 'x'} ${fmt(x)}`;
}

/** "1 value not drawn", "3 values not drawn". */
export function notDrawn(n: number, why: string): string {
  return `${n} ${n === 1 ? 'value' : 'values'} not drawn (${why})`;
}
