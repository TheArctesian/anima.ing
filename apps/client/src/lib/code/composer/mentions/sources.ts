import type { CodeIconName } from '../../icons/icons';
import type { Can } from '../draft';

/** Where a skill comes from, for the badge at the end of its row (03 "Popup design", badge text and icons). */
export type SkillOrigin = 'app' | 'repo' | 'project' | 'personal' | 'system' | 'other';

/** A skill the agent offers; `$name` in the prompt calls it. */
export interface Skill {
  readonly name: string;
  readonly displayName?: string;
  readonly shortDescription?: string;
  readonly description?: string;
  /** The agent's own word for where the skill lives: "user", "project". */
  readonly scope?: string;
  readonly origin: SkillOrigin;
}

/** A slash command (03 "What / lists"). */
export interface Command {
  readonly name: string;
  readonly description: string;
  /** Ours, not the agent's: built-in commands sort first. */
  readonly builtIn: boolean;
  /** Offered also when the `/` is not the first character of the prompt. */
  readonly anywhere: boolean;
  /** `text`: the choice writes `/name ` in the prompt. `act`: it removes the typed text and the composer acts. */
  readonly effect: 'text' | 'act';
  /** When it cannot be used now the row shows the reason and a choice does nothing. */
  readonly available: Can;
}

/** A file or folder the workspace search found, by its path from the workspace root. */
export interface FoundPath {
  readonly path: string;
  readonly folder: boolean;
}

/** A different chat that the prompt can refer to. */
export interface ThreadRef {
  readonly id: string;
  readonly title: string;
  /** "Thread", "Side chat", "Thread on studio". */
  readonly description: string;
}

/** What the lists of the mention menu hold (03 "What / lists" and the three sections after it). */
export interface MentionSources {
  /** For `/`, in the order to list them with an empty query: built-in first, then the agent's. */
  readonly commands: readonly Command[];
  /** For `$`, and for `/` when `skillsInCommands` is not false. */
  readonly skills: readonly Skill[];
  readonly skillsInCommands?: boolean;
  /** The agent still reads its skills: an empty `/` or `$` list says so. */
  readonly scanning?: boolean;
  /** For `@`, before the files; listed only with a query, five at most. */
  readonly threads?: readonly ThreadRef[];
  /** For `@`: the workspace search, at most `limit` paths; an empty query lists the top of the workspace. */
  readonly files: (query: string, limit: number) => Promise<readonly FoundPath[]>;
  /** Whether the workspace can be searched at all; when it cannot, the `@` list says why and asks nothing. */
  readonly search?: Can;
}

/** The source draws all rows in one group with no header; the group says which list a row came from. */
export type MentionGroup = 'Commands' | 'Skills' | 'Threads' | 'Files and folders';

interface Row {
  /** Stable in a list: the option id is built from it. */
  readonly id: string;
  readonly group: MentionGroup;
  readonly label: string;
  /** The start of the label that is drawn quiet: `/skill:` before a skill's name in the `/` list. */
  readonly quiet?: string;
  readonly description: string;
  /** The source draws an icon for a file, a folder and a thread, none for a command or a skill. */
  readonly icon?: CodeIconName;
  readonly available: Can;
}

export interface SkillBadge {
  readonly icon: CodeIconName;
  readonly label: string;
}

/** One row of the menu, and what a choice of it means. */
export type MentionEntry =
  | (Row & { readonly kind: 'command'; readonly command: Command })
  | (Row & { readonly kind: 'skill'; readonly skill: Skill; readonly badge: SkillBadge })
  | (Row & { readonly kind: 'path'; readonly path: string; readonly folder: boolean })
  | (Row & { readonly kind: 'thread'; readonly thread: ThreadRef });
