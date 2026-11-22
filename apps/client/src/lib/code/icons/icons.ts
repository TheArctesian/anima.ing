import AlarmClock from '@lucide/svelte/icons/alarm-clock';
import AlarmClockOff from '@lucide/svelte/icons/alarm-clock-off';
import ArrowDown from '@lucide/svelte/icons/arrow-down';
import ArrowRightLeft from '@lucide/svelte/icons/arrow-right-left';
import ArrowUp from '@lucide/svelte/icons/arrow-up';
import Bot from '@lucide/svelte/icons/bot';
import Brain from '@lucide/svelte/icons/brain';
import ChartNoAxesColumn from '@lucide/svelte/icons/chart-no-axes-column';
import Check from '@lucide/svelte/icons/check';
import ChevronDown from '@lucide/svelte/icons/chevron-down';
import ChevronLeft from '@lucide/svelte/icons/chevron-left';
import ChevronRight from '@lucide/svelte/icons/chevron-right';
import ChevronUp from '@lucide/svelte/icons/chevron-up';
import CircleAlert from '@lucide/svelte/icons/circle-alert';
import CircleCheck from '@lucide/svelte/icons/circle-check';
import CircleDashed from '@lucide/svelte/icons/circle-dashed';
import CircleDot from '@lucide/svelte/icons/circle-dot';
import CircleDotDashed from '@lucide/svelte/icons/circle-dot-dashed';
import Clock from '@lucide/svelte/icons/clock';
import Copy from '@lucide/svelte/icons/copy';
import CornerUpRight from '@lucide/svelte/icons/corner-up-right';
import Ellipsis from '@lucide/svelte/icons/ellipsis';
import Eye from '@lucide/svelte/icons/eye';
import File from '@lucide/svelte/icons/file';
import FileCode from '@lucide/svelte/icons/file-code';
import FileSearch from '@lucide/svelte/icons/file-search';
import FileText from '@lucide/svelte/icons/file-text';
import Folder from '@lucide/svelte/icons/folder';
import FolderPlus from '@lucide/svelte/icons/folder-plus';
import GitBranch from '@lucide/svelte/icons/git-branch';
import GitMerge from '@lucide/svelte/icons/git-merge';
import GitPullRequest from '@lucide/svelte/icons/git-pull-request';
import GitPullRequestArrow from '@lucide/svelte/icons/git-pull-request-arrow';
import Globe from '@lucide/svelte/icons/globe';
import Hammer from '@lucide/svelte/icons/hammer';
import Image from '@lucide/svelte/icons/image';
import Keyboard from '@lucide/svelte/icons/keyboard';
import Link from '@lucide/svelte/icons/link';
import Link2 from '@lucide/svelte/icons/link-2';
import ListFilter from '@lucide/svelte/icons/list-filter';
import ListPlus from '@lucide/svelte/icons/list-plus';
import ListTodo from '@lucide/svelte/icons/list-todo';
import Maximize2 from '@lucide/svelte/icons/maximize-2';
import MessageCircle from '@lucide/svelte/icons/message-circle';
import MessageCircleQuestion from '@lucide/svelte/icons/message-circle-question';
import MessageSquare from '@lucide/svelte/icons/message-square';
import MessageSquareDashed from '@lucide/svelte/icons/message-square-dashed';
import Mic from '@lucide/svelte/icons/mic';
import MicOff from '@lucide/svelte/icons/mic-off';
import Minimize2 from '@lucide/svelte/icons/minimize-2';
import Monitor from '@lucide/svelte/icons/monitor';
import PanelBottom from '@lucide/svelte/icons/panel-bottom';
import PanelLeft from '@lucide/svelte/icons/panel-left';
import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
import PanelRight from '@lucide/svelte/icons/panel-right';
import Paperclip from '@lucide/svelte/icons/paperclip';
import Pencil from '@lucide/svelte/icons/pencil';
import Pin from '@lucide/svelte/icons/pin';
import PinOff from '@lucide/svelte/icons/pin-off';
import Play from '@lucide/svelte/icons/play';
import Plus from '@lucide/svelte/icons/plus';
import Redo2 from '@lucide/svelte/icons/redo-2';
import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
import Search from '@lucide/svelte/icons/search';
import Settings from '@lucide/svelte/icons/settings';
import Shield from '@lucide/svelte/icons/shield';
import ShieldCheck from '@lucide/svelte/icons/shield-check';
import ShieldOff from '@lucide/svelte/icons/shield-off';
import ShieldQuestion from '@lucide/svelte/icons/shield-question';
import Sparkles from '@lucide/svelte/icons/sparkles';
import Square from '@lucide/svelte/icons/square';
import SquareMenu from '@lucide/svelte/icons/square-menu';
import SquarePen from '@lucide/svelte/icons/square-pen';
import Terminal from '@lucide/svelte/icons/terminal';
import TextSearch from '@lucide/svelte/icons/text-search';
import Trash2 from '@lucide/svelte/icons/trash-2';
import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
import Undo2 from '@lucide/svelte/icons/undo-2';
import UserRound from '@lucide/svelte/icons/user-round';
import Video from '@lucide/svelte/icons/video';
import WrapText from '@lucide/svelte/icons/wrap-text';
import Wrench from '@lucide/svelte/icons/wrench';
import X from '@lucide/svelte/icons/x';
import Archive from '@lucide/svelte/icons/archive';
import Trash from '@lucide/svelte/icons/trash';
import ArrowLeft from '@lucide/svelte/icons/arrow-left';
import ArrowRight from '@lucide/svelte/icons/arrow-right';
import Zap from '@lucide/svelte/icons/zap';

/**
 * Every Lucide icon the Code section draws, by T3 Code's name for it (D4, operator 2026-11-21: "Lucide now"; ISC
 * licence in `LICENSE-Lucide.txt`). This is the one list of icons to redraw in Anima's own set later: add an icon here,
 * never import Lucide elsewhere (the lint refuses it).
 */
export const CODE_ICONS = {
  'arrow-right': ArrowRight,
  'arrow-left': ArrowLeft,
  'trash': Trash,
  'archive': Archive,
  'alarm-clock': AlarmClock,
  'alarm-clock-off': AlarmClockOff,
  'arrow-down': ArrowDown,
  'arrow-right-left': ArrowRightLeft,
  'arrow-up': ArrowUp,
  bot: Bot,
  brain: Brain,
  'chart-no-axes-column': ChartNoAxesColumn,
  check: Check,
  'chevron-down': ChevronDown,
  'chevron-left': ChevronLeft,
  'chevron-right': ChevronRight,
  'chevron-up': ChevronUp,
  'circle-alert': CircleAlert,
  'circle-check': CircleCheck,
  'circle-dashed': CircleDashed,
  'circle-dot': CircleDot,
  'circle-dot-dashed': CircleDotDashed,
  clock: Clock,
  copy: Copy,
  'corner-up-right': CornerUpRight,
  ellipsis: Ellipsis,
  eye: Eye,
  file: File,
  'file-code': FileCode,
  'file-search': FileSearch,
  'file-text': FileText,
  folder: Folder,
  'folder-plus': FolderPlus,
  'git-branch': GitBranch,
  'git-merge': GitMerge,
  'git-pull-request': GitPullRequest,
  'git-pull-request-arrow': GitPullRequestArrow,
  globe: Globe,
  hammer: Hammer,
  image: Image,
  keyboard: Keyboard,
  link: Link,
  'link-2': Link2,
  'list-filter': ListFilter,
  'list-plus': ListPlus,
  'list-todo': ListTodo,
  'maximize-2': Maximize2,
  'message-circle': MessageCircle,
  'message-circle-question': MessageCircleQuestion,
  'message-square': MessageSquare,
  'message-square-dashed': MessageSquareDashed,
  mic: Mic,
  'mic-off': MicOff,
  'minimize-2': Minimize2,
  monitor: Monitor,
  'panel-bottom': PanelBottom,
  'panel-left': PanelLeft,
  'panel-left-close': PanelLeftClose,
  'panel-right': PanelRight,
  paperclip: Paperclip,
  pencil: Pencil,
  pin: Pin,
  'pin-off': PinOff,
  play: Play,
  plus: Plus,
  'redo-2': Redo2,
  'rotate-ccw': RotateCcw,
  search: Search,
  settings: Settings,
  shield: Shield,
  'shield-check': ShieldCheck,
  'shield-off': ShieldOff,
  'shield-question': ShieldQuestion,
  sparkles: Sparkles,
  square: Square,
  'square-menu': SquareMenu,
  'square-pen': SquarePen,
  terminal: Terminal,
  'text-search': TextSearch,
  'trash-2': Trash2,
  'triangle-alert': TriangleAlert,
  'undo-2': Undo2,
  'user-round': UserRound,
  video: Video,
  'wrap-text': WrapText,
  wrench: Wrench,
  x: X,
  zap: Zap,
} as const;

export type CodeIconName = keyof typeof CODE_ICONS;
