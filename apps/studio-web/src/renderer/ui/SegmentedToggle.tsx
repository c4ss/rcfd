// Segmented toggle (DESIGN.md Components): joined ruled buttons sharing their 1px ink rules; the selected item is deep
// ink with white text. One item is always selected. Options that cannot be used here follow the group as Unavailable
// chips that say why.
import { ToggleButton, ToggleButtonGroup } from 'react-aria-components';
import { Unavailable, type UnavailableProps } from './Button.js';
import { cx } from './cx.js';
import { Icon } from './Pictogram.js';
import type { IconName } from './pictogramPaths.js';

export interface SegmentOption {
  id: string;
  label: string;
  icon?: IconName;
  /** A count after the label ("Cases 4"): data, so it is not a chrome word (F-833, src/shared/word-budget.ts). */
  count?: number;
}

export interface SegmentedToggleProps {
  /** Names the group for assistive technology (there is no visible group label in the DESIGN.md form). */
  'aria-label': string;
  options: readonly SegmentOption[];
  selected: string;
  onChange: (id: string) => void;
  unavailable?: readonly UnavailableProps[];
  className?: string;
}

export function SegmentedToggle({ options, selected, onChange, unavailable = [], className, ...aria }: SegmentedToggleProps) {
  return (
    <div className={cx('ui-segmented-wrap', className)}>
      <ToggleButtonGroup
        aria-label={aria['aria-label']}
        className="ui-segmented"
        selectionMode="single"
        disallowEmptySelection
        selectedKeys={[selected]}
        onSelectionChange={(keys) => {
          const k = [...keys][0];
          if (k !== undefined) onChange(String(k));
        }}
      >
        {options.map((o) => (
          <ToggleButton key={o.id} id={o.id} className="ui-segment">
            {o.icon && <Icon name={o.icon} />}
            {o.label}
            {o.count !== undefined && (
              <>
                {' '}
                <span data-words="data">{o.count}</span>
              </>
            )}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>
      {unavailable.map((u) => (
        <Unavailable key={u.label} {...u} />
      ))}
    </div>
  );
}
