// Tooltip (DESIGN.md: deep ink with white label type, 6px by 8px, square).
import type { ReactNode } from 'react';
import { Tooltip as AriaTooltip, type TooltipProps as AriaTooltipProps } from 'react-aria-components';

export interface TooltipProps extends Omit<AriaTooltipProps, 'className' | 'style' | 'children'> {
  children: ReactNode;
}

export function Tooltip({ children, offset = 6, ...rest }: TooltipProps) {
  return (
    <AriaTooltip {...rest} offset={offset} className="ui-tooltip">
      {children}
    </AriaTooltip>
  );
}
