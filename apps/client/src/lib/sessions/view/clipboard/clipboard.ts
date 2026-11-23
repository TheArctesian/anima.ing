import type { SessionPolicy } from '../../transfer';

export interface ClipboardRules {
  readonly copy: boolean;
  readonly paste: boolean;
  readonly notice: string | undefined;
  readonly pasteOff: string | undefined;
}

const UNREAD = 'Copy and paste are off: the hub did not answer.';
const UNREAD_PASTE = 'Off: the hub did not answer';
const OFF_FOR_DEVICE = 'Off for this device';
const BOTH_OFF = 'Copy and paste are off for this device.';
const COPY_OFF = 'Copy from the device is off.';
const PASTE_OFF = 'Paste to the device is off.';

export function clipboardRules(policy: SessionPolicy): ClipboardRules {
  if (!policy.read) return { copy: false, paste: false, notice: UNREAD, pasteOff: UNREAD_PASTE };
  const copy = policy.copy === true;
  const paste = policy.paste === true;
  const notice = copy ? (paste ? undefined : PASTE_OFF) : paste ? COPY_OFF : BOTH_OFF;
  return { copy, paste, notice, pasteOff: paste ? undefined : OFF_FOR_DEVICE };
}

export const NO_CLIPBOARD: ClipboardRules = { copy: false, paste: false, notice: undefined, pasteOff: undefined };
