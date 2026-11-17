// The theme control (F-818): an icon in the header band (no visible word at rest, G22: the setting is normal state)
// that opens the list Light, Dark, High contrast, System in the popover of the Select primitive. Its accessible name is
// "Theme" and the button reads the theme chosen ("Light"), so it is found by role and name (G13). The same setting is
// the command palette's Theme command (view.theme).
import { Button as AriaButton, ListBox, ListBoxItem, Popover, Select as AriaSelect, SelectValue, type Key } from 'react-aria-components';
import { isThemeSetting, THEME_LABEL, THEME_SETTINGS } from '../../shared/theme.js';
import { Icon } from '../ui/Pictogram.js';
import { setTheme, useThemeSetting } from './theme.js';
import './theme.css';

const OPTIONS = THEME_SETTINGS.map((id) => ({ id, label: THEME_LABEL[id] }));

export function ThemeSelect() {
  const setting = useThemeSetting();
  return (
    <AriaSelect
      aria-label="Theme"
      className="th-select"
      value={setting}
      onChange={(k: Key | null) => {
        if (isThemeSetting(k)) void setTheme(k);
      }}
    >
      <AriaButton className="ui-button ui-button--ghost ui-button--icon th-button">
        <Icon name="theme" />
        <SelectValue className="sr-only" />
      </AriaButton>
      <Popover className="ui-popover th-popover" offset={2} placement="bottom end">
        <ListBox className="ui-listbox" items={OPTIONS}>
          {(o) => (
            <ListBoxItem id={o.id} textValue={o.label} className="ui-option">
              {({ isSelected }) => (
                <>
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
