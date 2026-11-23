import { BuilderShellError, type BuilderShell } from '$lib/desktop/builder/shell';
import type { HubResult } from '$lib/hub/hub';
import type { BuildsApi, BuildTarget, ClaimedJob, JobFile, UploadUrl } from './api';
import { buildFeed } from './feed';

/** Where the release bucket's files are public (DNS-007); the hub checks each feed URL against its own base (BLD-065). */
export const RELEASES_BASE = 'https://releases.anima.ing';
/** The release folder of each target, as the hub names it (BLD-060). */
const FOLDERS: Readonly<Record<BuildTarget, string>> = { LINUX_X86_64: 'linux-x86_64', DARWIN_UNIVERSAL: 'darwin-universal', WINDOWS_X86_64: 'windows-x86_64' };
/** The platform of each target in the proposed feed (BLD-067). */
const PLATFORMS = { LINUX_X86_64: 'linux', DARWIN_UNIVERSAL: 'macos', WINDOWS_X86_64: 'windows' } as const;

/** The hub refused or could not take a call of the upload; only this fixed text is kept (LOG-007). */
export class UploadRefusedError extends Error {
  constructor() {
    super('The hub did not take the upload.');
    this.name = 'UploadRefusedError';
  }
}

function accepted<T>(result: HubResult<T>): T {
  if (!result.ok) throw new UploadRefusedError();
  return result.value;
}

function uploadsFor(files: readonly JobFile[], uploads: readonly UploadUrl[]): UploadUrl[] {
  const byName = files.map((file) => uploads.filter((upload) => upload.name === file.name));
  // An answer with more uploads than files is not the hub's own; it is refused rather than half used.
  if (uploads.length !== files.length || byName.some((found) => found.length !== 1)) throw new UploadRefusedError();
  return byName.flat();
}

/** What the upload needs; the date of the feed comes from `now` (CQ-021). */
export interface UploadPorts {
  readonly api: Pick<BuildsApi, 'uploadUrls' | 'reportUploaded'>;
  readonly shell: Pick<BuilderShell, 'releaseFiles' | 'upload'>;
  readonly now: () => Date;
}

/** The hub publishes from the report (BLD-060 to BLD-063); no file is sent once `stopping` is true (BLD-026). */
export async function uploadRelease(job: ClaimedJob, ports: UploadPorts, stopping: () => boolean): Promise<void> {
  const release = await ports.shell.releaseFiles(job);
  const files = release.files;
  const uploads = uploadsFor(files, accepted(await ports.api.uploadUrls(job.id, files)));
  for (const upload of uploads) {
    if (stopping()) throw new BuilderShellError('cancelled');
    await ports.shell.upload(job, upload);
  }
  const feed = buildFeed({
    version: job.version,
    baseUrl: `${RELEASES_BASE}/${FOLDERS[job.target]}/${job.version}`,
    date: ports.now(),
    built: { [PLATFORMS[job.target]]: { file: release.updater } },
  });
  accepted(await ports.api.reportUploaded(job.id, files, JSON.stringify(feed)));
}
