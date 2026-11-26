// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import type { SessionTunnel } from '../../../tunnel/guacamole/tunnel';
import { fillDisplay } from '../fit';
import RemoteDisplay from '../RemoteDisplay.svelte';
import { LEAST_PAD, PAD, SCREEN_SHARE, trackpadLayout } from './trackpad';

const MAC = { width: 3024, height: 1964 };

function fitted(pane: { width: number; height: number }, remote = MAC) {
  return trackpadLayout(pane, remote, fillDisplay(remote, pane));
}

describe('RA-050: the room beside the display is a trackpad', () => {
  it('in portrait a wide remote keeps its whole height and grows until the trackpad below it is small', () => {
    const { screen, pad, scale } = fitted({ width: 390, height: 700 });
    expect(screen).toEqual({ width: 390, height: 700 - PAD });
    expect(pad).toEqual({ x: 0, y: 700 - PAD, width: 390, height: PAD });
    expect(MAC.height * scale).toBeCloseTo(700 - PAD, 6);
    expect(MAC.width * scale).toBeGreaterThan(390);
  });

  it('in landscape a wide remote keeps its whole height, and the trackpad beside it takes the rest', () => {
    const pane = { width: 800, height: 340 };
    const { screen, pad, scale } = fitted(pane);
    expect(scale).toBe(fillDisplay(MAC, pane));
    expect(screen.height).toBe(340);
    expect(screen.width).toBeCloseTo(523.5, 1);
    expect(pad).toMatchObject({ x: screen.width, y: 0, height: 340 });
    expect((pad?.x ?? 0) + (pad?.width ?? 0)).toBeCloseTo(800, 6);
  });

  it('in landscape a tall remote keeps its whole width and grows until the trackpad beside it is small', () => {
    const tall = { width: 1080, height: 1920 };
    const { screen, pad, scale } = fitted({ width: 800, height: 340 }, tall);
    expect(screen).toEqual({ width: 800 - PAD, height: 340 });
    expect(pad).toEqual({ x: 800 - PAD, y: 0, width: PAD, height: 340 });
    expect(tall.width * scale).toBeCloseTo(800 - PAD, 6);
  });

  it('a display that fills the pane, as an RDP desktop sized to it does, leaves no trackpad', () => {
    const pane = { width: 390, height: 700 };
    expect(fitted(pane, pane)).toEqual({ screen: pane, scale: 1 });
  });

  it('room smaller than a thumb stays part of the pane, and a little more room is all trackpad', () => {
    const pane = { width: 400, height: 250 + LEAST_PAD - 1 };
    expect(fitted(pane, { width: 400, height: 250 }).pad).toBeUndefined();
    expect(fitted({ width: 400, height: 250 + LEAST_PAD }, { width: 400, height: 250 }).pad?.height).toBe(LEAST_PAD);
  });

  it('before the remote size is known, the display has the whole pane', () => {
    const pane = { width: 390, height: 700 };
    expect(trackpadLayout(pane, { width: 0, height: 0 }, 1)).toEqual({ screen: pane, scale: 1 });
    expect(trackpadLayout(pane, MAC, Number.NaN).pad).toBeUndefined();
  });
});

describe('RA-050: the screen is always bigger than the trackpad', () => {
  it('in landscape a remote whose whole height leaves a trackpad bigger than the screen grows past its height', () => {
    const remote = { width: 1024, height: 768 };
    const { screen, pad, scale } = fitted({ width: 844, height: 300 }, remote);
    expect(screen).toEqual({ width: 844 * SCREEN_SHARE, height: 300 });
    expect(pad).toMatchObject({ x: screen.width, width: 844 - screen.width, height: 300 });
    expect(remote.width * scale).toBeCloseTo(screen.width, 6);
    expect(remote.height * scale).toBeGreaterThan(300);
  });

  it('on a phone and a tablet, in each orientation, for each shape of remote', () => {
    const panes = [{ width: 390, height: 700 }, { width: 700, height: 390 }, { width: 844, height: 280 }, { width: 820, height: 1100 }, { width: 1100, height: 820 }];
    const remotes = [MAC, { width: 1024, height: 768 }, { width: 1000, height: 1000 }, { width: 1080, height: 1920 }, { width: 640, height: 480 }];
    for (const pane of panes) {
      for (const remote of remotes) {
        const { screen, pad } = fitted(pane, remote);
        if (pad === undefined) continue;
        expect(screen.width * screen.height, `${JSON.stringify(remote)} in ${JSON.stringify(pane)}`).toBeGreaterThan(pad.width * pad.height);
      }
    }
  });

  it('a remote that would grow past its own pixels keeps them, in the middle of the screen', () => {
    expect(trackpadLayout({ width: 820, height: 1100 }, { width: 800, height: 600 }, 1)).toMatchObject({
      scale: 1, screen: { height: 1100 * SCREEN_SHARE }, pad: { height: 1100 * (1 - SCREEN_SHARE) },
    });
  });
});

/** The display as a touch screen shows it, in a pane of `pane`; the remote is the Mac above at its fitted scale. */
function shown(pane: { width: number; height: number }, mobile = true) {
  const target = document.createElement('div');
  target.setPointerCapture = vi.fn();
  target.getBoundingClientRect = () => DOMRect.fromRect({ x: 0, y: 0, ...pane });
  document.body.append(target);
  const sendMouseState = vi.fn<(state: { x: number; y: number; left: boolean }) => void>();
  const element = document.createElement('div');
  const scale = fillDisplay(MAC, pane);
  const display = { getElement: () => element, getWidth: () => MAC.width, getHeight: () => MAC.height, getScale: () => scale, scale: vi.fn<(scale: number) => void>(), showCursor: vi.fn() };
  const client = { getDisplay: () => display, sendMouseState } as unknown as SessionTunnel['client'];
  const view = mount(RemoteDisplay, { target, props: { client, pane: target, paneSize: pane, remote: MAC, scale, mobile, interactive: true } });
  flushSync();
  return {
    pad: target.querySelector<HTMLElement>('[data-trackpad]'),
    screen: target.querySelector<HTMLElement>('[data-display]')?.parentElement,
    shownScale: () => display.scale.mock.lastCall?.[0],
    sendMouseState,
    close: () => {
      void unmount(view);
      target.remove();
    },
  };
}

describe('RA-050: the trackpad on the remote display', () => {
  it('a phone in portrait shows a small trackpad under the display, and the display grows to the trackpad', () => {
    const { pad, screen, shownScale, close } = shown({ width: 390, height: 700 });
    try {
      const top = Number.parseFloat(pad?.style.insetBlockStart ?? '');
      expect(top).toBe(700 - PAD);
      expect(pad?.style.blockSize).toBe(`${String(PAD)}px`);
      expect(MAC.height * (shownScale() ?? 0)).toBeCloseTo(top, 6);
      expect(pad?.style.inlineSize).toBe('390px');
      expect(pad?.textContent).toContain('Trackpad');
      expect(pad?.getAttribute('aria-hidden')).toBe('true');
      expect(Number.parseFloat(screen?.style.blockSize ?? '')).toBe(top);
    } finally {
      close();
    }
  });

  it('a finger that slides on the trackpad moves the remote pointer', () => {
    const { pad, sendMouseState, close } = shown({ width: 390, height: 700 });
    try {
      const touch = (type: string, clientY: number) =>
        pad?.dispatchEvent(new PointerEvent(type, { pointerType: 'touch', pointerId: 3, clientX: 200, clientY, bubbles: true, cancelable: true }));
      touch('pointerdown', 400);
      touch('pointermove', 440);
      touch('pointerup', 440);
      const [state] = sendMouseState.mock.lastCall ?? [];
      expect(state).toMatchObject({ x: MAC.width / 2, left: false });
      expect(state?.y).toBeGreaterThan(MAC.height / 2);
    } finally {
      close();
    }
  });

  it('a desktop pane, and a phone whose display fills it, have no trackpad', () => {
    for (const view of [shown({ width: 390, height: 700 }, false), shown({ width: 800, height: 520 })]) {
      expect(view.pad).toBeNull();
      expect(view.screen?.style.blockSize).toBe('');
      view.close();
    }
  });
});
