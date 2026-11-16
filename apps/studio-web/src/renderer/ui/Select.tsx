// Select (DESIGN.md Components): a field-styled button with a caret that opens a listbox in the one lifted surface,
// the popover (1px ink border, ink-tinted lift). On the header band it takes an ink border over sky-wash (onBand).
import type { ReactNode } from 'react';
import {
  Button as AriaButton,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Select as AriaSelect,
  SelectValue,
  Text,
  type Key,
} from 'react-aria-components';
import { cx } from './cx.js';
import { Icon } from './Pictogram.js';

export interface SelectOption {
  id: string;
  /** The text of the option, used for type-ahead and for the accessible name. */
  label: string;
  /** Shown before the label (a state pictogram, for example). Decorative. */
  lead?: ReactNode;
  isDisabled?: boolean;
}

export interface SelectProps {
  label?: string;
  'aria-label'?: string;
  description?: ReactNode;
  options: readonly SelectOption[];
  selected: string | null;
  onChange: (id: string) => void;
  placeholder?: string;
  onBand?: boolean;
  isDisabled?: boolean;
  className?: string;
}

export function Select({ label, description, options, selected, onChange, placeholder, onBand, isDisabled, className, ...aria }: SelectProps) {
  return (
    <AriaSelect
      aria-label={aria['aria-label']}
      className={cx('ui-field', 'ui-select', onBand && 'ui-select--band', className)}
      value={selected}
      onChange={(k: Key | null) => {
        if (k !== null) onChange(String(k));
      }}
      placeholder={placeholder}
      isDisabled={isDisabled}
      disabledKeys={options.filter((o) => o.isDisabled).map((o) => o.id)}
    >
      {label && <Label className="ui-label">{label}</Label>}
      <AriaButton className="ui-select-button">
        <SelectValue className="ui-select-value" />
        <Icon name="caret-down" className="ui-select-caret" />
      </AriaButton>
      {description && (
        <Text slot="description" className="ui-description">
          {description}
        </Text>
      )}
      <Popover className="ui-popover" offset={2}>
        <ListBox className="ui-listbox" items={options}>
          {(o) => (
            <ListBoxItem id={o.id} textValue={o.label} className="ui-option">
              {({ isSelected }) => (
                <>
                  {o.lead}
                  <span className="ui-option-label">{o.label}</span>
                  {isSelected && <Icon name="check" className="ui-option-check" />}
                </>
              )}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}
