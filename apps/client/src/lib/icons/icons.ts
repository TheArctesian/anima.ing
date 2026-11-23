import { ICON_SHAPES } from './shapes';

/**
 * Iconify ids: Carbon for interface icons, Simple Icons for operating-system and brand marks (UI-014, UI-016).
 * `design/bin/icons.ts` copies the shapes of every named icon into `shapes.ts`.
 */
export type IconName = keyof typeof ICON_SHAPES;

export type IconPart = readonly [tag: string, attributes: Readonly<Record<string, string>>];

export interface IconShape {
  readonly width: number;
  readonly height: number;
  readonly parts: readonly IconPart[];
}

export function iconShape(name: IconName): IconShape {
  return ICON_SHAPES[name];
}
