import { describe, expect, it } from 'vitest';
import { brandMark, composeImages, imageNames } from './mark';

describe('brandMark', () => {
  it('gives the mark of a known program by its own name, in any case or spelling', () => {
    expect(brandMark(['gitea', 'Gitea'], [])).toEqual({ icon: 'simple-icons:gitea' });
    expect(brandMark(['Uptime Kuma'], [])).toEqual({ icon: 'simple-icons:uptimekuma' });
    expect(brandMark(['uptime-kuma'], [])).toEqual({ icon: 'simple-icons:uptimekuma' });
  });

  it('knows a program by the image when its name is the operator’s own', () => {
    expect(brandMark(['passwords'], ['vaultwarden/server:1.32.0'])).toEqual({ icon: 'simple-icons:vaultwarden' });
    expect(brandMark(['media'], ['lscr.io/linuxserver/jellyfin:latest'])).toEqual({ icon: 'simple-icons:jellyfin' });
    expect(brandMark(['photos'], ['ghcr.io/immich-app/immich-server@sha256:0a1b'])).toEqual({ icon: 'simple-icons:immich' });
  });

  it('draws a project by its program before its database', () => {
    expect(brandMark(['photos'], ['postgres:16', 'ghcr.io/immich-app/immich-server:v1'])).toEqual({ icon: 'simple-icons:immich' });
    expect(brandMark(['db'], ['postgres:16'])).toEqual({ icon: 'simple-icons:postgresql' });
  });

  it('falls back to the first letter of the name when no mark exists', () => {
    expect(brandMark(['tracks'], ['owntracks/recorder:0.9', 'postgres:16'])).toEqual({ letter: 'T' });
    expect(brandMark(['heimdall'], ['lscr.io/linuxserver/heimdall:2.6'])).toEqual({ letter: 'H' });
    expect(brandMark(['', 'dawarich'], [])).toEqual({ letter: 'D' });
  });
});

describe('imageNames', () => {
  it('leaves out the registry, the tag and the digest', () => {
    expect(imageNames('ghcr.io/home-assistant/home-assistant:stable')).toEqual(['home-assistant', 'home-assistant']);
    expect(imageNames('localhost:5000/team/app@sha256:ff')).toEqual(['app', 'team']);
    expect(imageNames('redis')).toEqual(['redis']);
  });
});

describe('composeImages', () => {
  it('reads each image line of a Compose file', () => {
    const file = 'services:\n  web:\n    image: "gitea/gitea@sha256:aa"\n  db:\n    image: postgres:16 # pinned\n';
    expect(composeImages(file)).toEqual(['gitea/gitea@sha256:aa', 'postgres:16']);
  });
});
