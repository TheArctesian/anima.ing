/** Reads Codex's code-mode `exec` calls, JavaScript that runs its tools, into the command, edit or search a person would name. Pure. */

import type { ToolCall } from '../thread';

function literals(source: string, name: string): string[] {
  const found: string[] = [];
  for (const match of source.matchAll(new RegExp(`\\b${name}"?\\s*:\\s*("(?:[^"\\\\]|\\\\.)*")`, 'g'))) {
    try {
      found.push(JSON.parse(match[1] ?? '""') as string);
    } catch {
      // A literal JSON cannot read (a JavaScript-only escape) is left out; the script stays under the row.
    }
  }
  return found;
}

function patched(source: string): string[] {
  return [...source.matchAll(/\*\*\* (?:Add|Update|Delete) File: ([^\n\\`]+)/g)].map((match) => (match[1] ?? '').trim());
}

function base(path: string): string {
  return path.split('/').filter((part) => part !== '').at(-1) ?? path;
}

function andMore(names: readonly string[]): string {
  const [first = '', ...rest] = names;
  return rest.length === 0 ? first : `${first} and ${String(rest.length)} more`;
}

/** The output a person reads: the `output` of each chunk the script printed, without the script's own header. */
export function execOutput(text: string | null): { readonly output: string | null; readonly failed: boolean } {
  if (text === null) return { output: null, failed: false };
  const failed = text.startsWith('Script failed');
  const body = text.includes('Output:\n') ? text.slice(text.indexOf('Output:\n') + 'Output:\n'.length) : text;
  let exit = 0;
  const lines = body.split('\n').map((line) => {
    if (!line.startsWith('{"chunk_id"')) return line;
    try {
      const chunk = JSON.parse(line) as { output?: unknown; exit_code?: unknown };
      if (typeof chunk.exit_code === 'number' && chunk.exit_code !== 0) exit = chunk.exit_code;
      return typeof chunk.output === 'string' ? chunk.output.replace(/\n$/, '') : '';
    } catch {
      return line;
    }
  });
  const output = lines.join('\n').trim();
  return { output: output === '' || output === '{}' ? null : output, failed: failed || exit !== 0 };
}

/** The script cell a call left running ("Script running with cell ID 1"), or that a `wait` polls; `null` for neither. */
export function cellOf(tool: ToolCall): string | null {
  if (tool.name === 'wait') {
    try {
      const cell = (JSON.parse(tool.input) as { cell_id?: unknown }).cell_id;
      return typeof cell === 'string' ? cell : null;
    } catch {
      return null;
    }
  }
  return /^Script running with cell ID (\S+)/.exec(tool.output ?? '')?.[1] ?? null;
}

export function waited(ran: ToolCall, wait: ToolCall): ToolCall {
  const { output, failed } = execOutput(wait.output);
  const both = [ran.output, output].filter((part) => part !== null).join('\n');
  return { ...ran, output: both === '' ? null : both, failed: ran.failed || failed };
}

/** A tool call of Codex as a person would name it; `null` for a poll that carries nothing for a person. */
export function readExec(tool: ToolCall): ToolCall | null {
  if (tool.name === 'wait' && tool.input.includes('cell_id')) return null;
  if (tool.name !== 'exec') return tool;
  const source = tool.input;
  const commands = literals(source, 'cmd');
  const files = patched(source);
  const searches = literals(source, 'q');
  const { output, failed } = execOutput(tool.output);
  const read = { ...tool, output, failed: tool.failed || failed };
  if (commands.length > 0 && files.length === 0) return { ...read, kind: 'command', summary: andMore(commands) };
  if (files.length > 0) return { ...read, kind: 'edit', summary: commands.length === 0 ? andMore(files.map(base)) : `${andMore(files.map(base))}; ran ${andMore(commands)}` };
  if (searches.length > 0) return { ...read, kind: 'web', summary: andMore(searches) };
  const polls = /tools\.write_stdin\(\{[^}]*chars\s*:\s*""/.test(source) && !/tools\.(?!write_stdin\b)\w+\(/.test(source);
  if (polls) return output === null ? null : { ...read, kind: 'command', summary: 'Read more output' };
  const used = [...new Set([...source.matchAll(/tools\.(\w+)\(/g)].map((match) => match[1] ?? ''))].filter((name) => name !== '');
  return { ...read, kind: 'other', summary: used.length === 0 ? 'Ran a script' : `Ran a script: ${used.join(', ')}` };
}
