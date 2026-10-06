// The table is one tab stop: a row has the keyboard, and the arrow keys move
// it. These put the keyboard back there after a menu or a dialog.

/** Focuses a row by its id. False when it is not in the table any more. */
export function focusRow(id: string): boolean {
  const row = document.querySelector<HTMLElement>(
    `tr[data-row-id="${CSS.escape(id)}"]`,
  );
  row?.focus();
  return row !== null;
}

/**
 * Puts the keyboard back on a row when its menu closes, unless something
 * else has it by then. A menu also closes when the search box is clicked or
 * a dialog opens, and those must keep the focus they were given.
 */
export function returnFocusToRow(id: string, closingMenu: Element): void {
  const row = document.querySelector<HTMLElement>(
    `tr[data-row-id="${CSS.escape(id)}"]`,
  );
  const active = document.activeElement;
  const nobodyHasIt =
    active === null ||
    active === document.body ||
    // Still in the menu that is closing. Another row's menu, opened by the
    // click that closed this one, has the focus by right.
    closingMenu.contains(active) ||
    // The row's own menu button, when it was used to close the menu.
    (row?.contains(active) ?? false);
  if (nobodyHasIt) {
    row?.focus();
  }
}

/** Focuses the row Tab would land on. */
export function focusTabStopRow(): void {
  document.querySelector<HTMLElement>('tr[data-row-id][tabindex="0"]')?.focus();
}
