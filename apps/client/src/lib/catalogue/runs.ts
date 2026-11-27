import { composeImages } from '$lib/brands/mark';
import type { CatalogueEntry } from './api';

/** A machine as the Catalogue needs it: its name and the images of the containers that the hub read on it. */
export interface MachineImages {
  readonly name: string;
  readonly images: readonly string[];
}

/**
 * The repository of an image with no registry, tag or digest, and with the `library/` of Docker Hub left out:
 * `lscr.io/linuxserver/jellyfin:latest` is `linuxserver/jellyfin`, `docker.io/library/postgres:16` is `postgres`.
 */
export function repository(image: string): string {
  const path = (image.split('@')[0] ?? '').replace(/:[^/]*$/, '');
  const parts = path.split('/');
  const named = parts.length > 1 && /[.:]|^localhost$/.test(parts[0] ?? '') ? parts.slice(1) : parts;
  return (named[0] === 'library' && named.length === 2 ? named.slice(1) : named).join('/').toLowerCase();
}

/** An image of LinuxServer.io packs the program that its last name says (R-319), so it matches that program's own image. */
function sameProgram(entryImage: string, running: string): boolean {
  if (entryImage === running) return true;
  const [owner, program] = entryImage.split('/');
  return owner === 'linuxserver' && program !== undefined && running.split('/').at(-1) === program;
}

/**
 * HSE-022: the machines where the program of an entry runs, by the image of its first service, from the containers
 * that the hub read. Until the hub keeps the record of the installs (task H6), this is what runs, not what Anima installed.
 */
export function runsOn(entry: Pick<CatalogueEntry, 'composeFile'>, machines: readonly MachineImages[]): readonly string[] {
  const first = composeImages(entry.composeFile)[0];
  if (first === undefined) return [];
  const wanted = repository(first);
  return machines.filter((machine) => machine.images.some((image) => sameProgram(wanted, repository(image)))).map((machine) => machine.name);
}
