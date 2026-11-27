import { describe, expect, it } from 'vitest';
import { repository, runsOn } from './runs';

describe('repository', () => {
  it('leaves out the registry, the tag, the digest and the library of Docker Hub', () => {
    expect(repository('lscr.io/linuxserver/jellyfin:latest')).toBe('linuxserver/jellyfin');
    expect(repository('docker.io/library/postgres:16')).toBe('postgres');
    expect(repository('gitea/gitea@sha256:aa90')).toBe('gitea/gitea');
    expect(repository('localhost:5000/team/app:1')).toBe('team/app');
  });
});

describe('runsOn (HSE-022)', () => {
  const machines = [
    { name: 'pop-os', images: ['postgres:16', 'jellyfin/jellyfin:10.10'] },
    { name: 'nas', images: ['gitea/gitea:1.22'] },
    { name: 'mini', images: [] },
  ];

  it('names each machine that runs the image of the first service, at any tag', () => {
    expect(runsOn({ composeFile: 'services:\n  gitea:\n    image: gitea/gitea@sha256:aa\n' }, machines)).toEqual(['nas']);
    expect(runsOn({ composeFile: 'services:\n  db:\n    image: postgres@sha256:bb\n' }, machines)).toEqual(['pop-os']);
  });

  it('takes an image of LinuxServer.io for the program that it packs', () => {
    expect(runsOn({ composeFile: 'services:\n  jellyfin:\n    image: lscr.io/linuxserver/jellyfin@sha256:cc\n' }, machines)).toEqual(['pop-os']);
  });

  it('names no machine for an entry with no image, or one that runs nowhere', () => {
    expect(runsOn({ composeFile: '' }, machines)).toEqual([]);
    expect(runsOn({ composeFile: 'services:\n  app:\n    image: owntracks/recorder@sha256:dd\n' }, machines)).toEqual([]);
  });
});
