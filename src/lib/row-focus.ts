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

/** Focuses the row Tab would land on. */
export function focusTabStopRow(): void {
  document.querySelector<HTMLElement>('tr[data-row-id][tabindex="0"]')?.focus();
}
