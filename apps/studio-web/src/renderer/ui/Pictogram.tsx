// State pictograms and interface icons (DESIGN.md Shapes). A pictogram is decorative to assistive technology: it always
// stands beside its word (StateMark), which carries the state; colour only reinforces it (G8).
import { ICON_PATHS, markShape, markWord, type IconName, type MarkState, type Shape } from './pictogramPaths.js';
import { cx } from './cx.js';

function ShapeEl({ shape }: { shape: Shape }) {
  return 'circle' in shape ? <circle {...shape.circle} /> : <path d={shape.path} />;
}

export interface StatePictogramProps {
  /** One of DESIGN.md's five run states, or paused or cancelled (F-806, proposed for DESIGN.md). */
  state: MarkState;
  /** Rows that are not selected fade: the pictogram is half-filled (Fade by Colour). A failure never fades. */
  faded?: boolean;
  size?: number;
  className?: string;
}

export function StatePictogram({ state, faded = false, size = 14, className }: StatePictogramProps) {
  const isFaded = faded && state !== 'failed';
  return (
    <svg
      className={cx('ui-picto', `ui-picto--${state}`, isFaded && 'ui-picto--faded', className)}
      width={size}
      height={size}
      viewBox="0 0 14 14"
      aria-hidden="true"
      focusable="false"
      data-state={state}
    >
      <ShapeEl shape={markShape(state)} />
    </svg>
  );
}

export interface StateMarkProps extends StatePictogramProps {
  /** Replaces the state word, e.g. "converged · 13.2 s" (it must still start with the state word). */
  label?: string;
}

/** Pictogram plus word: the only way a run state is shown. */
export function StateMark({ state, faded, label, className, size }: StateMarkProps) {
  const isFaded = faded && state !== 'failed';
  return (
    <span className={cx('ui-state', `ui-state--${state}`, isFaded && 'ui-state--faded', className)}>
      <StatePictogram state={state} faded={faded} size={size} />
      <span className="ui-state-word">{label ?? markWord(state)}</span>
    </span>
  );
}

export interface IconProps {
  name: IconName;
  size?: number;
  className?: string;
}

/** An interface icon, decorative: the control around it carries the name (a visible word or aria-label). */
export function Icon({ name, size = 14, className }: IconProps) {
  const icon = ICON_PATHS[name];
  return (
    <svg
      className={cx('ui-icon', icon.filled && 'ui-icon--filled', className)}
      width={size}
      height={size}
      viewBox="0 0 14 14"
      aria-hidden="true"
      focusable="false"
      data-icon={name}
    >
      <path d={icon.path} />
    </svg>
  );
}
