import { MODEL_JUMPS, THREAD_JUMPS, type KeyGroup } from './keymap';

/** The words of the key table: a label for each command of T3 Code's table, its group, and where a rule acts. */

/** The command of a rule, with its words for the key table. */
export interface CommandWords {
  readonly label: string;
  readonly group: KeyGroup;
}

/** The words of each command, in the order of the key table within its group. */
export const COMMAND_WORDS: Readonly<Record<string, CommandWords>> = {
  'commandPalette.toggle': { label: 'Open or close the command palette', group: 'general' },
  'sidebar.toggle': { label: 'Show or hide the chat list', group: 'general' },
  'workspace.focusNext': { label: 'Focus the next area', group: 'general' },
  'workspace.focusPrevious': { label: 'Focus the previous area', group: 'general' },
  'navigation.back': { label: 'Back', group: 'general' },
  'navigation.forward': { label: 'Forward', group: 'general' },
  'filePicker.toggle': { label: 'Go to file', group: 'general' },
  'projectSearch.toggle': { label: 'Search project contents', group: 'general' },
  'editor.openFavorite': { label: 'Open in the editor', group: 'general' },
  'view.reopenClosed': { label: 'Reopen the last closed tab', group: 'general' },
  'chat.new': { label: 'New chat', group: 'threads' },
  'chat.newLocal': { label: 'New chat here, without asking for a project', group: 'threads' },
  'chat.newWithoutProject': { label: 'New chat without a project', group: 'threads' },
  'chat.find': { label: 'Find in this chat', group: 'threads' },
  'thread.previous': { label: 'Previous chat', group: 'threads' },
  'thread.next': { label: 'Next chat', group: 'threads' },
  ...Object.fromEntries(
    THREAD_JUMPS.map((rule, index) => [rule.command, { label: `Chat ${String(index + 1)} in the list`, group: 'threads' }]),
  ),
  'thread.copyReference': { label: 'Copy a link to this chat', group: 'threads' },
  'thread.settle': { label: 'Settle this chat', group: 'threads' },
  'thread.pin': { label: 'Pin or unpin this chat', group: 'threads' },
  'thread.undo': { label: 'Undo the last pin, settle, snooze or archive', group: 'threads' },
  'thread.steerQueuedMessage': { label: 'Send the first queued message now', group: 'threads' },
  'thread.editQueuedMessage': { label: 'Edit the last queued message', group: 'composer' },
  'composer.sendAlternate': { label: 'Send with the other follow-up (queue or steer)', group: 'composer' },
  'composer.sendBackground': { label: 'Start in the background', group: 'composer' },
  'composer.sendAndNewThread': { label: 'Send and start a new chat', group: 'composer' },
  'composer.dictation': { label: 'Start or finish dictation', group: 'composer' },
  'composer.stash': { label: 'Put the draft aside', group: 'composer' },
  'modelPicker.toggle': { label: 'Choose a model', group: 'composer' },
  'modelPicker.previousProvider': { label: 'Previous provider in the model picker', group: 'composer' },
  'modelPicker.nextProvider': { label: 'Next provider in the model picker', group: 'composer' },
  ...Object.fromEntries(
    MODEL_JUMPS.map((rule, index) => [rule.command, { label: `Model ${String(index + 1)} in the picker`, group: 'composer' }]),
  ),
  'composer.host': { label: 'Choose a machine', group: 'composer' },
  'composer.effort': { label: 'Choose the effort', group: 'composer' },
  'composer.mode': { label: 'Choose the permissions', group: 'composer' },
  'composer.workspace': { label: 'Choose the workspace', group: 'composer' },
  'composer.branch': { label: 'Choose the Git branch', group: 'composer' },
  'composer.previousWorktree': { label: 'Use the previous worktree', group: 'composer' },
  'pullRequest.copyNumber': { label: 'Copy the pull request number', group: 'composer' },
  'terminal.toggle': { label: 'Show or hide the terminal', group: 'terminal' },
  'terminal.new': { label: 'New terminal', group: 'terminal' },
  'terminal.split': { label: 'Split the terminal', group: 'terminal' },
  'terminal.splitVertical': { label: 'Split the terminal down', group: 'terminal' },
  'terminal.close': { label: 'Close the terminal', group: 'terminal' },
  'rightPanel.toggle': { label: 'Show or hide the right panel', group: 'panels' },
  'rightPanel.new': { label: 'New tab in the right panel', group: 'panels' },
  'rightPanel.close': { label: 'Close the tab of the right panel', group: 'panels' },
  'threadPanel.toggle': { label: 'Show or hide the chat details', group: 'panels' },
  'diff.toggle': { label: 'Show or hide the diff', group: 'panels' },
  'preview.toggle': { label: 'Show or hide the browser', group: 'browser' },
  'preview.refresh': { label: 'Reload the page', group: 'browser' },
  'preview.focusUrl': { label: 'Go to the address', group: 'browser' },
  'preview.zoomIn': { label: 'Zoom in', group: 'browser' },
  'preview.zoomOut': { label: 'Zoom out', group: 'browser' },
  'preview.resetZoom': { label: 'Actual size', group: 'browser' },
  'preview.find': { label: 'Find on the page', group: 'browser' },
  'usage.open': { label: 'Open agent usage', group: 'usage' },
  'usage.cost': { label: 'Show cost', group: 'usage' },
  'usage.tokens': { label: 'Show tokens', group: 'usage' },
  'usage.limits': { label: 'Show limits', group: 'usage' },
  'usage.period.day': { label: 'One day', group: 'usage' },
  'usage.period.week': { label: 'One week', group: 'usage' },
  'usage.period.month': { label: 'One month', group: 'usage' },
  'usage.period.quarter': { label: 'Three months', group: 'usage' },
};

/** The groups of the key table, in its order, with their headings. */
export const KEY_GROUPS: readonly { readonly id: KeyGroup; readonly label: string }[] = [
  { id: 'general', label: 'General' },
  { id: 'threads', label: 'Chats' },
  { id: 'composer', label: 'Composer' },
  { id: 'terminal', label: 'Terminal' },
  { id: 'panels', label: 'Panels' },
  { id: 'browser', label: 'Browser' },
  { id: 'usage', label: 'Agent usage' },
];

/** The words for each condition of the default table; a condition not listed here is shown as written. */
const WHEN_WORDS: Readonly<Record<string, string>> = {
  '!terminalFocus': 'Not in a terminal',
  terminalFocus: 'In a terminal',
  previewFocus: 'In the browser',
  '!terminalFocus && !previewFocus': 'Not in a terminal or the browser',
  'rightPanelOpen && !terminalFocus': 'Right panel open, not in a terminal',
  composerFocus: 'In the composer',
  'composerFocus && turnRunning': 'In the composer while the agent works',
  'composerFocus && draftThreadRoute': 'In the composer of a new chat',
  'composerFocus && !draftThreadRoute': 'In the composer of a chat',
  modelPickerOpen: 'Model picker open',
  '!terminalFocus && !editableFocus': 'Not while typing',
  '!modelPickerOpen && isDesktop': 'Desktop app',
  'modelPickerOpen && isDesktop': 'Desktop app, model picker open',
  usagePageOpen: 'On the agent usage page',
};

export function whenWords(when: string | undefined): string {
  if (when === undefined) return 'Anywhere';
  return WHEN_WORDS[when] ?? when;
}
