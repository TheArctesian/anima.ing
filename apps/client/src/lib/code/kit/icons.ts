import type { IconName } from '$lib/icons/icons';

/** The icon of our drawn set that stands for one of T3 Code's, and the Carbon icon it should have when we lack one. */
export interface CodeIcon {
  readonly icon: IconName;
  /** A Carbon id that is not in `shapes.ts` yet: `icon` stands in for it until decision D4 is ruled. */
  readonly wanted?: string;
}

/** T3 Code's icons (Lucide names) that the Code section shows, by the name of the place that shows them. */
export const CODE_ICONS: Readonly<Record<string, CodeIcon>> = {
  CircleDashed: { icon: 'carbon:circle-dash' },
  CircleDotDashed: { icon: 'carbon:time', wanted: 'carbon:in-progress' },
  ShieldQuestion: { icon: 'carbon:security' },
  Shield: { icon: 'carbon:security' },
  MessageCircleQuestion: { icon: 'carbon:information', wanted: 'carbon:help' },
  UserRound: { icon: 'carbon:user' },
  CircleAlert: { icon: 'carbon:warning' },
  TriangleAlert: { icon: 'carbon:warning-alt' },
  AlarmClock: { icon: 'carbon:notification', wanted: 'carbon:alarm' },
  AlarmClockOff: { icon: 'carbon:notification', wanted: 'carbon:notification-off' },
  CircleCheck: { icon: 'carbon:checkmark-outline' },
  Check: { icon: 'carbon:checkmark' },
  Clock: { icon: 'carbon:time' },
  Terminal: { icon: 'carbon:terminal' },
  SquarePen: { icon: 'carbon:edit' },
  Eye: { icon: 'carbon:document', wanted: 'carbon:view' },
  Wrench: { icon: 'carbon:tool-kit' },
  Hammer: { icon: 'carbon:connect', wanted: 'carbon:tools' },
  Ellipsis: { icon: 'carbon:overflow-menu-vertical', wanted: 'carbon:overflow-menu-horizontal' },
  ChevronDown: { icon: 'carbon:chevron-down' },
  ChevronRight: { icon: 'carbon:chevron-right' },
  X: { icon: 'carbon:close' },
  Search: { icon: 'carbon:search' },
  Plus: { icon: 'carbon:add' },
  Copy: { icon: 'carbon:copy' },
  Folder: { icon: 'carbon:folder' },
  Settings: { icon: 'carbon:settings' },
  GitBranch: { icon: 'carbon:branch' },
  GitMerge: { icon: 'carbon:merge' },
  GitPullRequest: { icon: 'carbon:merge', wanted: 'carbon:pull-request' },
  CircleDot: { icon: 'carbon:circle-dash', wanted: 'carbon:dot-mark' },
  Pin: { icon: 'carbon:tag', wanted: 'carbon:pin' },
  PinOff: { icon: 'carbon:tag', wanted: 'carbon:pin-filled' },
  Undo2: { icon: 'carbon:arrow-left', wanted: 'carbon:undo' },
  ArrowUp: { icon: 'carbon:arrow-up' },
  Square: { icon: 'carbon:pause', wanted: 'carbon:stop-filled' },
  Paperclip: { icon: 'carbon:link', wanted: 'carbon:attachment' },
  Mic: { icon: 'carbon:activity', wanted: 'carbon:microphone' },
  PanelLeft: { icon: 'carbon:side-panel-close', wanted: 'carbon:side-panel-open' },
  ArrowRightLeft: { icon: 'carbon:arrows-vertical', wanted: 'carbon:arrows-horizontal' },
};
