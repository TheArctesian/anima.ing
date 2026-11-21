import AlarmClock from '@lucide/svelte/icons/alarm-clock';
import AlarmClockOff from '@lucide/svelte/icons/alarm-clock-off';
import ArrowRightLeft from '@lucide/svelte/icons/arrow-right-left';
import ArrowUp from '@lucide/svelte/icons/arrow-up';
import Check from '@lucide/svelte/icons/check';
import ChevronDown from '@lucide/svelte/icons/chevron-down';
import ChevronRight from '@lucide/svelte/icons/chevron-right';
import CircleAlert from '@lucide/svelte/icons/circle-alert';
import CircleCheck from '@lucide/svelte/icons/circle-check';
import CircleDashed from '@lucide/svelte/icons/circle-dashed';
import CircleDot from '@lucide/svelte/icons/circle-dot';
import CircleDotDashed from '@lucide/svelte/icons/circle-dot-dashed';
import Clock from '@lucide/svelte/icons/clock';
import Copy from '@lucide/svelte/icons/copy';
import Ellipsis from '@lucide/svelte/icons/ellipsis';
import Eye from '@lucide/svelte/icons/eye';
import Folder from '@lucide/svelte/icons/folder';
import GitBranch from '@lucide/svelte/icons/git-branch';
import GitMerge from '@lucide/svelte/icons/git-merge';
import GitPullRequest from '@lucide/svelte/icons/git-pull-request';
import Hammer from '@lucide/svelte/icons/hammer';
import Maximize2 from '@lucide/svelte/icons/maximize-2';
import MessageCircleQuestion from '@lucide/svelte/icons/message-circle-question';
import Mic from '@lucide/svelte/icons/mic';
import Minimize2 from '@lucide/svelte/icons/minimize-2';
import PanelBottom from '@lucide/svelte/icons/panel-bottom';
import PanelLeft from '@lucide/svelte/icons/panel-left';
import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
import PanelRight from '@lucide/svelte/icons/panel-right';
import Paperclip from '@lucide/svelte/icons/paperclip';
import Pin from '@lucide/svelte/icons/pin';
import PinOff from '@lucide/svelte/icons/pin-off';
import Plus from '@lucide/svelte/icons/plus';
import Search from '@lucide/svelte/icons/search';
import Settings from '@lucide/svelte/icons/settings';
import Shield from '@lucide/svelte/icons/shield';
import ShieldQuestion from '@lucide/svelte/icons/shield-question';
import Square from '@lucide/svelte/icons/square';
import SquareMenu from '@lucide/svelte/icons/square-menu';
import SquarePen from '@lucide/svelte/icons/square-pen';
import Terminal from '@lucide/svelte/icons/terminal';
import Trash2 from '@lucide/svelte/icons/trash-2';
import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
import Undo2 from '@lucide/svelte/icons/undo-2';
import UserRound from '@lucide/svelte/icons/user-round';
import Wrench from '@lucide/svelte/icons/wrench';
import X from '@lucide/svelte/icons/x';

/**
 * Every Lucide icon the Code section draws, by T3 Code's name for it (D4, operator 2026-11-21: "Lucide now"; ISC
 * licence in `LICENSE-Lucide.txt`). This is the one list of icons to redraw in Anima's own set later: add an icon here,
 * never import Lucide elsewhere (the lint refuses it).
 */
export const CODE_ICONS = {
  'alarm-clock': AlarmClock,
  'alarm-clock-off': AlarmClockOff,
  'arrow-right-left': ArrowRightLeft,
  'arrow-up': ArrowUp,
  check: Check,
  'chevron-down': ChevronDown,
  'chevron-right': ChevronRight,
  'circle-alert': CircleAlert,
  'circle-check': CircleCheck,
  'circle-dashed': CircleDashed,
  'circle-dot': CircleDot,
  'circle-dot-dashed': CircleDotDashed,
  clock: Clock,
  copy: Copy,
  ellipsis: Ellipsis,
  eye: Eye,
  folder: Folder,
  'git-branch': GitBranch,
  'git-merge': GitMerge,
  'git-pull-request': GitPullRequest,
  hammer: Hammer,
  'maximize-2': Maximize2,
  'message-circle-question': MessageCircleQuestion,
  mic: Mic,
  'minimize-2': Minimize2,
  'panel-bottom': PanelBottom,
  'panel-left': PanelLeft,
  'panel-left-close': PanelLeftClose,
  'panel-right': PanelRight,
  paperclip: Paperclip,
  pin: Pin,
  'pin-off': PinOff,
  plus: Plus,
  search: Search,
  settings: Settings,
  shield: Shield,
  'shield-question': ShieldQuestion,
  square: Square,
  'square-menu': SquareMenu,
  'square-pen': SquarePen,
  terminal: Terminal,
  'trash-2': Trash2,
  'triangle-alert': TriangleAlert,
  'undo-2': Undo2,
  'user-round': UserRound,
  wrench: Wrench,
  x: X,
} as const;

export type CodeIconName = keyof typeof CODE_ICONS;
