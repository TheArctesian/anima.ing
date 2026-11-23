/** The typefaces the operator can choose for each text role (UI-010, UI-011, UI-020). */
export const TEXT_ROLES = [
  {
    id: 'ui',
    label: 'Interface font',
    default: 'ibm-plex-sans',
    choices: [
      { id: 'ibm-plex-sans', label: 'IBM Plex Sans' },
      { id: 'opendyslexic', label: 'OpenDyslexic' },
    ],
  },
  {
    id: 'mono',
    label: 'Monospace font',
    default: 'ibm-plex-mono',
    choices: [
      { id: 'ibm-plex-mono', label: 'IBM Plex Mono' },
      { id: 'opendyslexic', label: 'OpenDyslexic' },
    ],
  },
] as const;

export type TextRole = (typeof TEXT_ROLES)[number]['id'];

type RoleSpec<R extends TextRole> = Extract<(typeof TEXT_ROLES)[number], { id: R }>;

export type Typeface<R extends TextRole = TextRole> = RoleSpec<R>['choices'][number]['id'];

export type TypefaceChoice = { readonly [R in TextRole]: Typeface<R> };

const storageKey = (role: TextRole): string => `anima.typeface.${role}`;

/** Storage can refuse reads in privacy modes. */
function readText(storage: Storage, role: TextRole): string | null {
  try {
    return storage.getItem(storageKey(role));
  } catch {
    return null;
  }
}

function chosen<R extends TextRole>(storage: Storage, role: RoleSpec<R>): Typeface<R> {
  const text = readText(storage, role.id);
  const found = role.choices.find((choice) => choice.id === text);
  return (found?.id ?? role.default) as Typeface<R>;
}

/** The typeface the operator chose for each role; each role's default when nothing valid is stored (UI-020). */
export function storedTypefaces(storage: Storage): TypefaceChoice {
  const [ui, mono] = TEXT_ROLES;
  return { ui: chosen(storage, ui), mono: chosen(storage, mono) };
}

export function storeTypeface<R extends TextRole>(storage: Storage, role: R, typeface: Typeface<R>): void {
  storage.setItem(storageKey(role), typeface);
}

/** `global.css` points the `font-ui` and `font-mono` tokens at these attributes (UI-020). */
export function applyTypefaces(root: HTMLElement, choice: TypefaceChoice): void {
  root.dataset['fontUi'] = choice.ui;
  root.dataset['fontMono'] = choice.mono;
}
