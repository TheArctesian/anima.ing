import type { Size } from '../../display';
import type { SessionTunnel } from '../../../tunnel/guacamole/tunnel';

type Client = Pick<SessionTunnel['client'], 'getDisplay' | 'sendMouseState'>;
interface Point { readonly x: number; readonly y: number }

export interface TouchViewport extends Point {
  readonly zoom: number;
}

export const FIT_VIEWPORT: TouchViewport = { x: 0, y: 0, zoom: 1 };
const MAX_ZOOM = 4;
const TAP_DISTANCE = 7;
const DRAG_HOLD_MS = 350;

function limit(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function boundAxis(position: number, leftover: number): number {
  return leftover > 0 ? leftover / 2 : limit(position, leftover, 0);
}

export function boundViewport(view: TouchViewport, pane: Size, remote: Size, scale: number): TouchViewport {
  return {
    zoom: view.zoom,
    x: boundAxis(view.x, pane.width - remote.width * scale * view.zoom),
    y: boundAxis(view.y, pane.height - remote.height * scale * view.zoom),
  };
}

interface Pinch {
  readonly center: Point;
  readonly distance: number;
  readonly view: TouchViewport;
}

function pinch(points: ReadonlyMap<number, Point>, view: TouchViewport): Pinch | undefined {
  const [first, second] = points.values();
  if (first === undefined || second === undefined) return undefined;
  return {
    center: { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 },
    distance: Math.hypot(second.x - first.x, second.y - first.y),
    view,
  };
}

function pinched(start: Pinch, next: Pinch): TouchViewport {
  if (start.distance === 0) return start.view;
  const zoom = limit(start.view.zoom * next.distance / start.distance, 1, MAX_ZOOM);
  const ratio = zoom / start.view.zoom;
  return {
    zoom,
    x: next.center.x - (start.center.x - start.view.x) * ratio,
    y: next.center.y - (start.center.y - start.view.y) * ratio,
  };
}

interface ViewportChanges {
  read(): TouchViewport;
  show(view: TouchViewport): void;
}

type Button = 'left' | 'right' | null;
type Send = (delta: Point, button: Button) => void;

function cursorSender(client: Client): Send {
  const display = client.getDisplay();
  let cursor: Point | undefined;
  return (delta, button) => {
    const width = display.getWidth();
    const height = display.getHeight();
    if (width <= 0 || height <= 0) return;
    const scale = display.getScale() || 1;
    const from = cursor ?? { x: width / 2, y: height / 2 };
    cursor = {
      x: limit(from.x + delta.x / scale, 0, width - 1),
      y: limit(from.y + delta.y / scale, 0, height - 1),
    };
    display.showCursor?.(true);
    client.sendMouseState({ ...cursor, left: button === 'left', right: button === 'right', middle: false, up: false, down: false }, false);
  };
}

interface TouchState {
  touches: Map<number, Point>;
  start?: { point: Point; at: number } | undefined;
  gesture?: Pinch | undefined;
  origin: Point;
  moved: boolean;
  dragged: boolean;
  pinching: boolean;
  cancelled: boolean;
  send: Send;
}

const STILL: Point = { x: 0, y: 0 };

function pointInPane(state: TouchState, event: PointerEvent): Point {
  return { x: event.clientX - state.origin.x, y: event.clientY - state.origin.y };
}

function down(state: TouchState, pane: HTMLElement, viewport: ViewportChanges, event: PointerEvent): void {
  if (event.pointerType !== 'touch' || state.touches.size >= 2) return;
  event.preventDefault();
  if (state.touches.size === 0) {
    const { left, top } = pane.getBoundingClientRect();
    state.origin = { x: left, y: top };
  }
  pane.setPointerCapture(event.pointerId);
  const point = pointInPane(state, event);
  state.touches.set(event.pointerId, point);
  if (state.touches.size === 1 && state.gesture === undefined) state.start = { point, at: event.timeStamp };
  if (state.touches.size === 2) {
    if (state.dragged) state.send(STILL, null);
    state.dragged = false;
    state.gesture = pinch(state.touches, viewport.read());
  }
}

function move(state: TouchState, viewport: ViewportChanges, event: PointerEvent): void {
  const previous = state.touches.get(event.pointerId);
  if (previous === undefined) return;
  const point = pointInPane(state, event);
  state.touches.set(event.pointerId, point);
  if (state.gesture !== undefined) {
    const next = pinch(state.touches, state.gesture.view);
    if (next === undefined) return;
    const centerTravel = Math.hypot(next.center.x - state.gesture.center.x, next.center.y - state.gesture.center.y);
    state.pinching ||= centerTravel >= TAP_DISTANCE || Math.abs(next.distance - state.gesture.distance) >= TAP_DISTANCE;
    if (state.pinching) viewport.show(pinched(state.gesture, next));
    return;
  }
  const start = state.start;
  if (start === undefined) return;
  if (!state.moved && Math.hypot(point.x - start.point.x, point.y - start.point.y) < TAP_DISTANCE) return;
  if (!state.moved && event.timeStamp - start.at >= DRAG_HOLD_MS) {
    state.dragged = true;
    state.send(STILL, 'left');
  }
  const from = state.moved ? previous : start.point;
  state.moved = true;
  state.send({ x: point.x - from.x, y: point.y - from.y }, state.dragged ? 'left' : null);
}

function end(state: TouchState, event: PointerEvent): void {
  if (!state.touches.delete(event.pointerId)) return;
  state.cancelled ||= event.type !== 'pointerup';
  if (state.touches.size > 0) return;
  if (!state.moved && !state.cancelled && !state.pinching) {
    state.send(STILL, state.gesture === undefined ? 'left' : 'right');
    state.send(STILL, null);
  } else if (state.dragged) {
    state.send(STILL, null);
  }
  state.start = undefined;
  state.gesture = undefined;
  state.moved = false;
  state.dragged = false;
  state.pinching = false;
  state.cancelled = false;
}

export function touchRemote(pane: HTMLElement, client: Client, viewport: ViewportChanges): () => void {
  const state: TouchState = {
    touches: new Map(), origin: STILL, moved: false, dragged: false, pinching: false, cancelled: false, send: cursorSender(client),
  };
  const listeners = new AbortController();
  const options = { signal: listeners.signal };
  const release = (event: PointerEvent): void => { end(state, event); };
  pane.addEventListener('pointerdown', (event) => { down(state, pane, viewport, event); }, options);
  pane.addEventListener('pointermove', (event) => { move(state, viewport, event); }, options);
  pane.addEventListener('pointerup', release, options);
  pane.addEventListener('pointercancel', release, options);
  pane.addEventListener('lostpointercapture', release, options);
  return () => {
    if (state.dragged) state.send(STILL, null);
    listeners.abort();
  };
}
