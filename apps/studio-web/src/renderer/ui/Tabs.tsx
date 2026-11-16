// Tabs (DESIGN.md Components, Navigation). Two forms:
//   sub      a 32px row ruled below in ink; the selected tab is a meadow field at 700 weight with a 3px ink bar
//   project  in the sky header band, 13px 500 weight, divided by 35% ink lines; the selected project is a white tab at
//            700 weight with ink side rules
// Arrow keys move between tabs and select them (automatic activation); Home and End go to the ends.
import type { ReactNode } from 'react';
import { Tab, TabList, TabPanel, Tabs as AriaTabs, type Key } from 'react-aria-components';
import { cx } from './cx.js';

export interface TabItem {
  id: string;
  label: string;
  /** The panel of this tab. Project tabs may have none (the page below is the panel). */
  panel?: ReactNode;
}

export interface TabsProps {
  'aria-label': string;
  variant?: 'sub' | 'project';
  items: readonly TabItem[];
  selected: string;
  onChange: (id: string) => void;
  className?: string;
}

export function Tabs({ variant = 'sub', items, selected, onChange, className, ...aria }: TabsProps) {
  return (
    <AriaTabs
      className={cx('ui-tabs', `ui-tabs--${variant}`, className)}
      selectedKey={selected}
      onSelectionChange={(k: Key) => onChange(String(k))}
    >
      <TabList aria-label={aria['aria-label']} className="ui-tablist" items={items}>
        {(t) => (
          <Tab id={t.id} className="ui-tab">
            {t.label}
          </Tab>
        )}
      </TabList>
      {items.map((t) =>
        t.panel === undefined ? null : (
          <TabPanel key={t.id} id={t.id} className="ui-tabpanel">
            {t.panel}
          </TabPanel>
        ),
      )}
    </AriaTabs>
  );
}
