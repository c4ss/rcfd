// Buttons (DESIGN.md Components, Buttons): plain, ruled and square. Default is a white face in a 1px ink rule; primary
// is deep ink with white text (New run, Send, Play); ghost has no border or fill (the header band); chip is the 22px
// reference chip. Focus is the product focus ring (tokens.ts, focus) on every variant.
import type { ReactNode } from 'react';
import {
  Button as AriaButton,
  Focusable,
  ToggleButton as AriaToggleButton,
  TooltipTrigger,
  type ButtonProps as AriaButtonProps,
  type ToggleButtonProps as AriaToggleButtonProps,
} from 'react-aria-components';
import { cx } from './cx.js';
import { Icon } from './Pictogram.js';
import type { IconName } from './pictogramPaths.js';
import { Tooltip } from './Tooltip.js';

export type ButtonVariant = 'default' | 'primary' | 'ghost' | 'chip';

export interface ButtonProps extends Omit<AriaButtonProps, 'className' | 'style' | 'children'> {
  variant?: ButtonVariant;
  icon?: IconName;
  className?: string;
  children?: ReactNode;
}

export function Button({ variant = 'default', icon, className, children, ...rest }: ButtonProps) {
  return (
    <AriaButton {...rest} className={cx('ui-button', `ui-button--${variant}`, className)}>
      {icon && <Icon name={icon} />}
      {children}
    </AriaButton>
  );
}

export interface IconButtonProps extends Omit<ButtonProps, 'children' | 'icon' | 'aria-label'> {
  icon: IconName;
  /** The accessible name, also shown as the tooltip. */
  label: string;
}

/** A button with an icon and no visible word: the name is its aria-label and its tooltip. */
export function IconButton({ icon, label, className, ...rest }: IconButtonProps) {
  return (
    <TooltipTrigger delay={600}>
      <Button {...rest} icon={icon} aria-label={label} className={cx('ui-button--icon', className)} />
      <Tooltip>{label}</Tooltip>
    </TooltipTrigger>
  );
}

export interface ToggleProps extends Omit<AriaToggleButtonProps, 'className' | 'style' | 'children'> {
  icon?: IconName;
  className?: string;
  children?: ReactNode;
}

/** A single ruled toggle ("Follow live"): white, deep ink with white text when on. */
export function Toggle({ icon, className, children, ...rest }: ToggleProps) {
  return (
    <AriaToggleButton {...rest} className={cx('ui-segment', 'ui-segment--single', className)}>
      {icon && <Icon name={icon} />}
      {children}
    </AriaToggleButton>
  );
}

export interface UnavailableProps {
  label: string;
  /** Why the option is unavailable; shown in a tooltip on hover and keyboard focus, and read as its description. */
  reason: string;
}

/**
 * An option that cannot be used here: a dashed silver chip in ink-2 that says why in a tooltip. It stays in the tab
 * order (a disabled control could not show its reason) and is announced as a disabled button.
 */
export function Unavailable({ label, reason }: UnavailableProps) {
  return (
    <TooltipTrigger delay={300}>
      <Focusable>
        <span className="ui-unavailable" role="button" aria-disabled="true" tabIndex={0}>
          {label}
        </span>
      </Focusable>
      <Tooltip>{reason}</Tooltip>
    </TooltipTrigger>
  );
}
