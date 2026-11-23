import type { ContainerImage, ContainerNetwork, ContainerStorage } from '../api';

// HOST-020: the images and the networks as rows of text; the volumes are in the section Storage of Hosting (HSE-015). A size is the text that Docker writes.

export type StorageKind = 'images' | 'networks';

/** The types of Docker whose space it counts: the images here, the volumes in the section Storage of Hosting. */
export type UsageKind = 'images' | 'volumes';

export interface StorageTable {
  readonly columns: readonly string[];
  readonly rows: readonly { readonly key: string; readonly cells: readonly string[] }[];
}

/**
 * The containers that use a thing: their names, with how many more Docker counts when the hub found not all of them;
 * the number alone when it found none; "Not used" for zero.
 */
export function usedBy(count: number | null | undefined, names: readonly string[]): string {
  if (names.length === 0) return count == null ? 'Not known' : count === 0 ? 'Not used' : `${String(count)} ${count === 1 ? 'container' : 'containers'}`;
  const more = (count ?? names.length) - names.length;
  return more > 0 ? `${names.join(', ')} and ${String(more)} more` : names.join(', ');
}

/** `repository:tag`; the repository alone for an image with no tag; the first digits of the identifier for one with no name. */
function imageName(image: ContainerImage): string {
  if (image.repository == null) return image.id.replace(/^sha256:/, '').slice(0, 12);
  return image.tag == null ? image.repository : `${image.repository}:${image.tag}`;
}

export function imageRows(images: readonly ContainerImage[]): StorageTable {
  return {
    columns: ['Image', 'Size', 'Only this image', 'Used by'],
    rows: images.map((image) => ({
      key: `${image.id} ${image.repository ?? ''} ${image.tag ?? ''}`,
      cells: [imageName(image), image.size, image.uniqueSize, usedBy(image.containerCount, image.containers)],
    })),
  };
}

export function networkRows(networks: readonly ContainerNetwork[]): StorageTable {
  return {
    columns: ['Network', 'Driver', 'Scope', 'Reach', 'Containers'],
    rows: networks.map((network) => ({
      key: network.id,
      cells: [network.name, network.driver, network.scope, network.internal ? 'Inside the device only' : 'Outside too', network.containers.length === 0 ? 'None' : network.containers.join(', ')],
    })),
  };
}

const USAGE_TYPE: Readonly<Record<UsageKind, string>> = { images: 'Images', volumes: 'Local Volumes' };

/** "3 images", "1 network". */
export function countWords(count: number, kind: StorageKind): string {
  return `${String(count)} ${count === 1 ? kind.slice(0, -1) : kind}`;
}

/** "470.4MB in use, 49.63MB (10%) can be freed by a clean-up" for the type; empty when Docker gave no line for it. */
export function usageLine(usage: ContainerStorage['usage'], kind: UsageKind): string {
  const line = usage.find((each) => each.type === USAGE_TYPE[kind]);
  return line === undefined ? '' : `${line.size} in use, ${line.reclaimable} can be freed by a clean-up`;
}
