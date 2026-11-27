import type { HubError } from '$lib/hub/hub';
import type { CommandReceipt, LaunchReceipt } from '../api';

export function launchWaiting({ binding, state }: LaunchReceipt): boolean {
  return binding === 'waiting' && ['queued', 'claimed', 'reconciling', 'started'].includes(state);
}

export function confirmedThread({ binding, thread }: LaunchReceipt): string | null {
  return binding === 'bound' ? thread ?? null : null;
}

export function commandWaiting({ state }: CommandReceipt): boolean {
  return state === 'waiting' || state === 'delivered';
}

function reason(cause: string | null | undefined): string {
  switch (cause) {
    case 'device_unavailable':
    case 'device_offline': return ' The computer is offline.';
    case 'folder_not_permitted': return ' The computer does not permit this project folder.';
    case 'work_off':
    case 'starts_off': return ' This computer has turned off remote starts.';
    case 'target_stale': return ' Choose a current target and try again.';
    case 'request_expired':
    case 'request_stale': return ' This request is no longer open on the computer.';
    default: return '';
  }
}

export function launchMessage(receipt: LaunchReceipt): string {
  if (receipt.binding === 'unavailable' && ['queued', 'claimed', 'reconciling'].includes(receipt.state)) {
    return 'The hub cannot confirm the start on this computer.';
  }
  switch (receipt.state) {
    case 'queued': return 'Waiting for the computer to accept the start.';
    case 'claimed': return 'The computer is starting the thread.';
    case 'reconciling': return 'Checking whether the computer started the thread.';
    case 'started': return receipt.binding === 'unavailable'
      ? 'The computer started the thread. The hub could not make it available.'
      : 'The computer started the thread. Waiting for it to appear on the hub.';
    case 'refused': return `The computer or hub refused the start.${reason(receipt.cause)}`;
    case 'failed': return `The computer could not start the thread.${reason(receipt.cause)}`;
    case 'stopped': return 'The start was stopped.';
    case 'expired': return 'The start expired before it was confirmed.';
    case 'retired': return 'The hub no longer has the start result.';
  }
}

export function commandMessage(receipt: CommandReceipt): string {
  switch (receipt.state) {
    case 'waiting': return 'Waiting for the computer.';
    case 'delivered': return 'Sent to the computer. Waiting for its result.';
    case 'recorded': return 'The computer confirmed the action.';
    case 'refused': return `The computer or hub refused the action.${reason(receipt.cause)}`;
    case 'expired': return 'The action expired before it was confirmed.';
    case 'retired': return 'The hub no longer has the action result.';
  }
}

export function writeRefused(error: HubError): boolean {
  return ['FORBIDDEN', 'UNAUTHENTICATED', 'VALIDATION', 'unserved'].includes(error.code)
    || (error.code === 'INTERNAL' && error.reason !== undefined);
}

export function remoteError(error: HubError): string {
  if (error.code !== 'INTERNAL') return error.message;
  switch (error.reason) {
    case 'target_unavailable': return 'This target is no longer available. Refresh the targets and choose again.';
    case 'COMMANDS_FULL': return 'The computer has too many pending actions. Wait for an earlier action to finish.';
    case 'WORK_OFF':
    case 'threads_off': return 'This computer is no longer accepting remote work.';
    case 'thread_unknown':
    case 'THREAD_NOT_FOUND': return 'This thread is unavailable for remote actions.';
    default: return reason(error.reason).trim() || error.message;
  }
}
