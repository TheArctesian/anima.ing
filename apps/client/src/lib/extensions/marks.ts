import type { IconName } from '$lib/icons/icons';

const MARKS: Readonly<Record<string, IconName>> = {
  projects: 'carbon:folder',
  integrations: 'carbon:link',
  map: 'carbon:location',
  writing: 'carbon:document',
  secrets: 'carbon:locked',
  people: 'carbon:user',
  money: 'carbon:dashboard',
  history: 'carbon:recently-viewed',
  health: 'carbon:activity',
  documents: 'carbon:certificate',
  reminders: 'carbon:notification',
  assistant_sessions: 'carbon:terminal',
  wardrobe: 'carbon:tag',
  library: 'carbon:catalog',
  model_gateway: 'carbon:connect',
};

/** Each picture comes from the synthetic hub's sample data, never an operator's (OD-EXT-NEW-pictures). */
const PICTURED: ReadonlySet<string> = new Set(['projects', 'integrations']);

export function pictureOf(key: string, built: boolean): { dark: string; light: string } | null {
  if (!built || !PICTURED.has(key)) return null;
  return { dark: `/extensions/${key}-dark.webp`, light: `/extensions/${key}-light.webp` };
}

export function markOf(key: string): IconName {
  return MARKS[key] ?? 'carbon:grid';
}

export const VIEW_KEY = 'anima.extensions.view';

export type ExtensionsView = 'list' | 'grid';

export function storedView(storage: Pick<Storage, 'getItem'> | undefined): ExtensionsView {
  try {
    return storage?.getItem(VIEW_KEY) === 'grid' ? 'grid' : 'list';
  } catch {
    return 'list';
  }
}

export function storeView(storage: Pick<Storage, 'setItem'> | undefined, view: ExtensionsView): void {
  try {
    storage?.setItem(VIEW_KEY, view);
  } catch {
    // The page still shows the view that was chosen.
  }
}
