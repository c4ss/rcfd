// Switch (DESIGN.md Components): a 28 by 14px ink-ruled track with a square knob; meadow when on. The knob's position
// carries the state, the fill reinforces it. The focus ring is drawn on the track.
import type { ReactNode } from 'react';
import { SwitchButton, SwitchField, Text, type SwitchFieldProps } from 'react-aria-components';
import { cx } from './cx.js';

export interface SwitchProps extends Omit<SwitchFieldProps, 'className' | 'style' | 'children'> {
  children: ReactNode;
  description?: ReactNode;
  className?: string;
}

export function Switch({ children, description, className, ...rest }: SwitchProps) {
  return (
    <SwitchField {...rest} className={cx('ui-switch-field', className)}>
      <SwitchButton className="ui-switch">
        <span className="ui-switch-track" aria-hidden="true">
          <span className="ui-switch-knob" />
        </span>
        <span className="ui-switch-label">{children}</span>
      </SwitchButton>
      {description && (
        <Text slot="description" className="ui-description">
          {description}
        </Text>
      )}
    </SwitchField>
  );
}
