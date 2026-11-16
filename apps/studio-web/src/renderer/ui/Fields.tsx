// Inputs and number fields (DESIGN.md Components, Inputs / Fields): white, 26px tall, square, ink-2 border at rest
// (ink on hover and focus), ink-2 placeholder, orange-deep caret. The unit goes in the label ("Length, m"), never after
// the value (the Tabular Figures Rule). A field always has a visible label or an aria-label.
import type { ReactNode } from 'react';
import {
  Button as AriaButton,
  FieldError,
  Group,
  Input,
  Label,
  NumberField as AriaNumberField,
  Text,
  TextField as AriaTextField,
  type NumberFieldProps as AriaNumberFieldProps,
  type TextFieldProps as AriaTextFieldProps,
} from 'react-aria-components';
import { cx } from './cx.js';
import { Icon } from './Pictogram.js';

interface FieldChrome {
  /** The visible label. Omit only together with aria-label. */
  label?: ReactNode;
  /** Help under the field (G19: explain in place). */
  description?: ReactNode;
  /** Shown when the field is invalid; it names the problem and the way to recover (G19). */
  errorMessage?: string;
  className?: string;
}

export interface TextFieldProps extends Omit<AriaTextFieldProps, 'className' | 'style' | 'children'>, FieldChrome {
  placeholder?: string;
}

export function TextField({ label, description, errorMessage, placeholder, className, ...rest }: TextFieldProps) {
  return (
    <AriaTextField {...rest} className={cx('ui-field', className)}>
      {label && <Label className="ui-label">{label}</Label>}
      <Input className="ui-input" placeholder={placeholder} />
      {description && (
        <Text slot="description" className="ui-description">
          {description}
        </Text>
      )}
      <FieldError className="ui-error">{errorMessage}</FieldError>
    </AriaTextField>
  );
}

export interface NumberFieldProps extends Omit<AriaNumberFieldProps, 'className' | 'style' | 'children'>, FieldChrome {}

/** A number input with step buttons; ArrowUp and ArrowDown step, Home and End go to the limits. */
export function NumberField({ label, description, errorMessage, className, ...rest }: NumberFieldProps) {
  return (
    <AriaNumberField {...rest} className={cx('ui-field', 'ui-number', className)}>
      {label && <Label className="ui-label">{label}</Label>}
      <Group className="ui-number-group">
        <Input className="ui-input" />
        <AriaButton slot="decrement" className="ui-number-step">
          <Icon name="minus" />
        </AriaButton>
        <AriaButton slot="increment" className="ui-number-step">
          <Icon name="plus" />
        </AriaButton>
      </Group>
      {description && (
        <Text slot="description" className="ui-description">
          {description}
        </Text>
      )}
      <FieldError className="ui-error">{errorMessage}</FieldError>
    </AriaNumberField>
  );
}
