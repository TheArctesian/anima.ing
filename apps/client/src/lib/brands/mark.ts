import type { IconName } from '$lib/icons/icons';

/**
 * The mark of a program in Hosting (HSE-020): its brand mark from Simple Icons (CC0, UI-014), else the first letter of
 * its name. The Library of the models draws the mark of a publisher by the same rule (task L1-20 of compute-plan.md).
 */
export type BrandMark = { readonly icon: IconName } | { readonly letter: string };

/**
 * The programs that Simple Icons has a mark for, by their name in lower case with no space, dash or dot. A program with
 * no mark in the pinned set (Heimdall, Dawarich, OwnTracks, ...) is not here and keeps its first letter.
 */
const PROGRAMS: Readonly<Record<string, IconName>> = {
  adguard: 'simple-icons:adguard',
  adguardhome: 'simple-icons:adguard',
  audiobookshelf: 'simple-icons:audiobookshelf',
  authelia: 'simple-icons:authelia',
  bitwarden: 'simple-icons:bitwarden',
  caddy: 'simple-icons:caddy',
  calibreweb: 'simple-icons:calibreweb',
  codeserver: 'simple-icons:coder',
  duplicati: 'simple-icons:duplicati',
  emby: 'simple-icons:emby',
  forgejo: 'simple-icons:forgejo',
  freshrss: 'simple-icons:freshrss',
  frigate: 'simple-icons:frigate',
  ghost: 'simple-icons:ghost',
  gitea: 'simple-icons:gitea',
  gitlab: 'simple-icons:gitlab',
  grafana: 'simple-icons:grafana',
  homeassistant: 'simple-icons:homeassistant',
  homarr: 'simple-icons:homarr',
  immich: 'simple-icons:immich',
  influxdb: 'simple-icons:influxdb',
  jellyfin: 'simple-icons:jellyfin',
  jenkins: 'simple-icons:jenkins',
  keycloak: 'simple-icons:keycloak',
  mariadb: 'simple-icons:mariadb',
  mealie: 'simple-icons:mealie',
  minio: 'simple-icons:minio',
  mongo: 'simple-icons:mongodb',
  mongodb: 'simple-icons:mongodb',
  mosquitto: 'simple-icons:eclipsemosquitto',
  mysql: 'simple-icons:mysql',
  n8n: 'simple-icons:n8n',
  nextcloud: 'simple-icons:nextcloud',
  nginx: 'simple-icons:nginx',
  nodered: 'simple-icons:nodered',
  ollama: 'simple-icons:ollama',
  paperlessngx: 'simple-icons:paperlessngx',
  pihole: 'simple-icons:pihole',
  plex: 'simple-icons:plex',
  portainer: 'simple-icons:portainer',
  postgres: 'simple-icons:postgresql',
  postgresql: 'simple-icons:postgresql',
  prometheus: 'simple-icons:prometheus',
  qbittorrent: 'simple-icons:qbittorrent',
  rabbitmq: 'simple-icons:rabbitmq',
  radarr: 'simple-icons:radarr',
  redis: 'simple-icons:redis',
  searxng: 'simple-icons:searxng',
  sonarr: 'simple-icons:sonarr',
  syncthing: 'simple-icons:syncthing',
  traefik: 'simple-icons:traefikproxy',
  transmission: 'simple-icons:transmission',
  uptimekuma: 'simple-icons:uptimekuma',
  vaultwarden: 'simple-icons:vaultwarden',
  watchtower: 'simple-icons:watchtower',
  wikijs: 'simple-icons:wikidotjs',
  wordpress: 'simple-icons:wordpress',
  zigbee2mqtt: 'simple-icons:zigbee2mqtt',
};

/** Programs that run beside an application (its database, its cache): a project is drawn by its own program first. */
const COMPANIONS: readonly string[] = ['mariadb', 'mongo', 'mongodb', 'mysql', 'postgres', 'postgresql', 'redis'];

/** The name of a program as PROGRAMS keys it: lower case, with no space, dash, dot or underscore. */
function key(words: string): string {
  return words.toLowerCase().replaceAll(/[^a-z0-9]/g, '');
}

/** The mark of one name: the whole name, else its first word ("immich-server" is Immich, "uptime-kuma" is Uptime Kuma). */
function markOf(name: string): IconName | undefined {
  const firstWord = name.split(/[\s_.-]+/)[0] ?? '';
  return PROGRAMS[key(name)] ?? PROGRAMS[key(firstWord)];
}

/**
 * The names of the program in an image reference, most telling first: the last part of the path, then the part before
 * it ("vaultwarden/server" is Vaultwarden). The registry, the tag and the digest are not names.
 */
export function imageNames(image: string): readonly string[] {
  const path = image.split('@')[0]?.replace(/:[^/]*$/, '') ?? '';
  const parts = path.split('/').filter((part, index, all) => !(index === 0 && all.length > 1 && /[.:]|^localhost$/.test(part)));
  return parts.reverse();
}

/** The images that a Compose file names (`image:` lines), in the order of the file. */
export function composeImages(composeFile: string): readonly string[] {
  return [...composeFile.matchAll(/^\s*image:\s*["']?([^\s"'#]+)/gm)].flatMap(([, image]) => (image === undefined ? [] : [image]));
}

/**
 * The mark of a thing from its own names (an entry, a project, a container) and the images it runs. A companion image
 * (a database, a cache) counts only when the thing runs nothing else, so a project of Immich and PostgreSQL is drawn as
 * Immich, and a project of OwnTracks and PostgreSQL keeps its first letter.
 */
export function brandMark(names: readonly string[], images: readonly string[]): BrandMark {
  const companion = (image: string) => imageNames(image).some((name) => COMPANIONS.includes(key(name)));
  const own = images.filter((image) => !companion(image));
  const candidates = [...names, ...(own.length > 0 ? own : images).flatMap(imageNames)];
  for (const name of candidates) {
    const icon = markOf(name);
    if (icon !== undefined) return { icon };
  }
  const first = names.find((name) => name.trim() !== '') ?? imageNames(images[0] ?? '')[0] ?? '?';
  return { letter: first.trim().charAt(0).toUpperCase() };
}
