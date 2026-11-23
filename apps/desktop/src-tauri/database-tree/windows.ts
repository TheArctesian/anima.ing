// Native PostgreSQL runtime, pinned official EDB and Microsoft distributions (WIN2).
// No installer is executed or installed globally.
import { spawnSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readdir, rename, rm } from 'node:fs/promises';
import { isAbsolute, join, resolve } from 'node:path';
import pin from './windows.pin.json' with { type: 'json' };

interface Archive { url: string; sha256: string }

const MINUTE = 60_000;
const WHOLE_BUILD_MINUTES = 25;
const ATTEMPTS = 2;

function checked(step: string, minutes: number, program: string, args: string[]): void {
  const result = spawnSync(program, args, { stdio: 'inherit', windowsHide: true, timeout: minutes * MINUTE });
  if ((result.error as NodeJS.ErrnoException | undefined)?.code === 'ETIMEDOUT') {
    throw new Error(`database-tree: ${step} took more than ${minutes} minutes and was stopped`);
  }
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) throw new Error(`database-tree: ${step} (${program}) did not finish successfully`);
}

async function archive(source: Archive, path: string, provided: string | undefined): Promise<Uint8Array> {
  if (provided !== undefined) {
    await copyFile(provided, path);
  } else {
    const response = await fetch(source.url, { signal: AbortSignal.timeout(600_000) });
    if (!response.ok) throw new Error(`database-tree: download failed (${response.status})`);
    await Bun.write(path, response);
  }
  const bytes = new Uint8Array(await Bun.file(path).arrayBuffer());
  const digest = new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
  if (digest !== source.sha256) throw new Error('database-tree: archive differs from its checked-in SHA-256');
  return bytes;
}

async function cabinet(bytes: Uint8Array, offset: number, length: number, path: string): Promise<void> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (offset + length > bytes.length || view.getUint32(offset, true) !== 0x4643534d || view.getUint32(offset + 8, true) !== length) {
    throw new Error('database-tree: the pinned cabinet layout is invalid');
  }
  await Bun.write(path, bytes.subarray(offset, offset + length));
}

async function visualCpp(work: string, tree: string, expand: string): Promise<void> {
  const runtime = pin.visualCpp;
  const bytes = await archive(runtime, join(work, 'vc-redist.exe'), process.env['ANIMA_VC_RUNTIME_ARCHIVE']);
  const payload = join(work, 'vc-payload');
  const bootstrap = join(work, 'vc-bootstrap');
  const dlls = join(work, 'vc-dlls');
  for (const folder of [payload, bootstrap, dlls]) await mkdir(folder);
  const payloadCab = join(work, 'payload.cab');
  const bootstrapCab = join(work, 'bootstrap.cab');
  await cabinet(bytes, runtime.payloadOffset, runtime.payloadBytes, payloadCab);
  await cabinet(bytes, runtime.bootstrapOffset, runtime.bootstrapBytes, bootstrapCab);
  checked('the expansion of the Visual C++ payload', 5, expand, ['-F:*', payloadCab, payload]);
  checked('the expansion of the Visual C++ bootstrap', 5, expand, ['-F:u4', bootstrapCab, bootstrap]);
  // These member names are part of the hash-pinned Microsoft 14.44.35211 distribution.
  checked('the expansion of the Visual C++ libraries', 5, expand, ['-F:*', join(payload, 'a12'), dlls]);
  const names = await readdir(dlls);
  if (names.length !== 12 || names.some((name) => !name.endsWith('.dll_amd64'))) {
    throw new Error('database-tree: the pinned x64 CRT cabinet has unexpected members');
  }
  for (const name of names) await copyFile(join(dlls, name), join(tree, 'bin', name.replace(/_amd64$/, '')));
  await copyFile(join(bootstrap, 'u4'), join(tree, 'visual-cpp-license.rtf'));
}

async function build(work: string, output: string): Promise<void> {
  const systemRoot = process.env['SystemRoot'];
  if (process.platform !== 'win32' || systemRoot === undefined || !isAbsolute(systemRoot)) {
    throw new Error('database-tree: this recipe requires native Windows');
  }
  const sourcePin = await Bun.file(new URL('./postgresql.pin', import.meta.url)).text();
  if (!sourcePin.split(/\r?\n/).includes(`version=${pin.postgresql.version}`)) {
    throw new Error('database-tree: Windows and Unix PostgreSQL versions differ');
  }
  const system = join(systemRoot, 'System32');
  await mkdir(work, { recursive: true });
  const temporary = await mkdtemp(join(work, 'postgres-'));
  try {
    const zip = join(temporary, 'postgresql.zip');
    await archive(pin.postgresql, zip, process.env['ANIMA_POSTGRESQL_WINDOWS_ARCHIVE']);
    checked('the extraction of PostgreSQL', 10, join(system, 'tar.exe'), ['-xf', zip, '-C', temporary,
      'pgsql/bin', 'pgsql/lib', 'pgsql/share', 'pgsql/server_license.txt', 'pgsql/commandlinetools_3rd_party_licenses.txt']);
    const tree = join(temporary, 'pgsql');
    await visualCpp(temporary, tree, join(system, 'expand.exe'));
    checked('the version check of PostgreSQL', 2, join(tree, 'bin/postgres.exe'), ['--version']);
    await Bun.write(join(tree, 'anima-runtime.json'), `${JSON.stringify(pin, null, 2)}\n`);
    // mkdir fails when the output exists, so stale runtime DLLs never merge into a new tree.
    await mkdir(output);
    try {
      for (const name of await readdir(tree)) await rename(join(tree, name), join(output, name));
    } catch (error) {
      // A partly moved tree is removed, so that a second attempt does not find the output.
      await rm(output, { recursive: true, force: true });
      throw error;
    }
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

const [work, output] = process.argv.slice(2);
if (work === undefined || output === undefined) throw new Error('usage: bun windows.ts <work-folder> <output-folder>');
// spawnSync blocks the timers, so each step has its own limit; this one bounds the waits between them.
setTimeout(() => {
  console.error(`database-tree: the build took more than ${WHOLE_BUILD_MINUTES} minutes and was stopped`);
  process.exit(1);
}, WHOLE_BUILD_MINUTES * MINUTE).unref();
for (let attempt = 1; ; attempt += 1) {
  try {
    await build(resolve(work), resolve(output));
    break;
  } catch (error) {
    if (attempt === ATTEMPTS) throw error;
    console.error(`database-tree: attempt ${attempt} failed (${error instanceof Error ? error.message : String(error)}); trying once more`);
  }
}
