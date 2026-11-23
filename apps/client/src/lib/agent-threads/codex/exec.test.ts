import { expect, it } from 'vitest';
import type { ToolCall } from '../thread';
import { cellOf, execOutput, readExec } from './exec';

const exec = (input: string, output: string | null = null): ToolCall => ({ name: 'exec', kind: 'other', summary: input, input, patch: null, output, failed: false });

it('reads an exec of commands as those commands, with the output the commands printed', () => {
  const tool = readExec(exec('text(await tools.exec_command({cmd:"git status --short",max_output_tokens:1000}));\ntext(await tools.exec_command({"cmd":"ls \\"a b\\""}));', 'Script completed\nWall time 1.0 seconds\nOutput:\n\n{"chunk_id":"1","exit_code":0,"output":"M a.ts\\n"}'));
  expect(tool).toMatchObject({ kind: 'command', summary: 'git status --short and 1 more', output: 'M a.ts', failed: false });
});

it('reads a patch as an edit of its files, and a failed script or exit code as a failure', () => {
  expect(readExec(exec('text(await tools.apply_patch("*** Begin Patch\\n*** Add File: /w/src/a.ts\\n+x\\n*** Update File: /w/b.ts\\n*** End Patch"));', 'Script failed\nexec failed'))).toMatchObject({ kind: 'edit', summary: 'a.ts and 1 more', failed: true });
  expect(execOutput('Script completed\nOutput:\n{"chunk_id":"1","exit_code":2,"output":"no"}')).toEqual({ output: 'no', failed: true });
});

it('leaves out a wait and an empty poll, and finds the cell a running script left', () => {
  const wait: ToolCall = { ...exec(''), name: 'wait', input: '{"cell_id":"g7:17","yield_time_ms":20000}' };
  expect(readExec(wait)).toBeNull();
  expect(cellOf(wait)).toBe('g7:17');
  expect(cellOf(exec('x', 'Script running with cell ID 3\nWall time 30 seconds'))).toBe('3');
  expect(readExec(exec('text(await tools.write_stdin({session_id:5,chars:"",yield_time_ms:1000}));', 'Script completed\nOutput:\n{}'))).toBeNull();
});
