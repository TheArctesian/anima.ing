import type { Program } from '$lib/agent-threads/thread';
import type { CodeIconName } from './icons';

/** T3 Code's `ProviderInstanceIcon.tsx`: each program by the mark of its maker. */
export const PROGRAM_ICONS: Readonly<Record<Program, CodeIconName>> = { claude_code: 'claude', codex: 'openai', grok: 'grok' };
