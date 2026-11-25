// The rows of the overlay fixture, shared by the Svelte and the React page so that both draw the same menu: one plain
// row with a shortcut, a disabled row between two enabled ones, and a row that opens a dialog (and so keeps the focus).
export interface FixtureItem {
  id: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  shortcut?: string;
  testId?: string;
  opensDialog?: boolean;
}

export function fixtureItems(select: (id: string) => void): FixtureItem[] {
  return [
    { id: 'alpha', label: 'Alpha', shortcut: 'A', testId: 'fx-alpha', onSelect: () => select('alpha') },
    { id: 'bravo', label: 'Bravo', disabled: true, testId: 'fx-bravo', onSelect: () => select('bravo') },
    { id: 'charlie', label: 'Charlie', testId: 'fx-charlie', onSelect: () => select('charlie') },
    { id: 'dialog', label: 'Dialog…', testId: 'fx-dialog', opensDialog: true, onSelect: () => select('dialog') },
  ];
}
