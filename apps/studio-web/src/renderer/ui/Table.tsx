// Table (DESIGN.md Components, Runs table): sortable columns, 48px rows ruled by silver hairlines. The selected row
// turns sky-wash, boxed with inset ink lines; a failed row keeps its fail-wash. Thin, styled wrappers of the React Aria
// table parts, so surfaces (F-806 runs table and later) compose their own columns and cells.
//
// Keyboard: arrows move between rows and cells, Space or Enter selects a row, Enter on a sortable column head sorts.
import type { ReactNode } from 'react';
import {
  Cell as AriaCell,
  Column as AriaColumn,
  Row as AriaRow,
  Table as AriaTable,
  TableBody as AriaTableBody,
  TableHeader as AriaTableHeader,
  type CellProps,
  type ColumnProps as AriaColumnProps,
  type RowProps,
  type TableBodyProps,
  type TableHeaderProps,
  type TableProps as AriaTableProps,
} from 'react-aria-components';
import { cx } from './cx.js';
import { Icon } from './Pictogram.js';

export interface TableProps extends Omit<AriaTableProps, 'className' | 'style'> {
  className?: string;
}

export function Table({ className, ...rest }: TableProps) {
  return <AriaTable {...rest} className={cx('ui-table', className)} />;
}

export function TableHeader<T extends object>({ className, ...rest }: Omit<TableHeaderProps<T>, 'className'> & { className?: string }) {
  return <AriaTableHeader {...rest} className={cx('ui-thead', className)} />;
}

export interface ColumnProps extends Omit<AriaColumnProps, 'className' | 'style' | 'children'> {
  children: ReactNode;
  className?: string;
}

/** A column head; a sortable one shows its direction with the sort icons (and aria-sort for assistive technology). */
export function Column({ children, className, ...rest }: ColumnProps) {
  return (
    <AriaColumn {...rest} className={cx('ui-column', className)}>
      {({ allowsSorting, sortDirection }) => (
        <span className="ui-column-head">
          {children}
          {allowsSorting && (
            <Icon
              name={sortDirection === 'ascending' ? 'sort-ascending' : sortDirection === 'descending' ? 'sort-descending' : 'sort'}
              size={10}
              className={cx('ui-sort', sortDirection && 'ui-sort--on')}
            />
          )}
        </span>
      )}
    </AriaColumn>
  );
}

export function TableBody<T extends object>({ className, ...rest }: Omit<TableBodyProps<T>, 'className'> & { className?: string }) {
  return <AriaTableBody {...rest} className={cx('ui-tbody', className)} />;
}

export interface UiRowProps<T extends object> extends Omit<RowProps<T>, 'className'> {
  /** A failed row keeps its fail-wash ground whether selected or not (failures never fade). */
  failed?: boolean;
  /** Not the selected run: its text turns ink-2 (the Fade by Colour Rule). Ignored for a failed row. */
  faded?: boolean;
  className?: string;
}

export function Row<T extends object>({ failed, faded, className, ...rest }: UiRowProps<T>) {
  return <AriaRow {...rest} className={cx('ui-row', failed && 'ui-row--failed', faded && !failed && 'ui-row--faded', className)} />;
}

export function Cell({ className, ...rest }: Omit<CellProps, 'className'> & { className?: string }) {
  return <AriaCell {...rest} className={cx('ui-cell', className)} />;
}
