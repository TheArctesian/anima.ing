import type { TransitionConfig } from 'svelte/transition';
import { easeCurve, springCurve } from '../animate';
import { modeTokens, springMotion, staggerDelayMs, type SpringName } from '../options';
import { motionPreference, type MotionPreference } from '../preference';
import { firstDirection, opacity, scaledFade } from './path';
import { roleScale } from './scales';

/* Use each role with `transition:`, not `in:` and `out:`, so a reversal starts from where the element is (MOT-002). */

export interface RoleParams {
  readonly preference?: MotionPreference;
}

export interface PopoverParams extends RoleParams {
  readonly from?: Element;
}

export interface ListRowParams extends RoleParams {
  readonly index?: number;
}

export type RoleTransition = (options?: { direction?: 'in' | 'out' }) => TransitionConfig;

function originAt(control: Element, surface: Element): string {
  const from = control.getBoundingClientRect();
  const box = surface.getBoundingClientRect();
  const x = from.left + from.width / 2 - box.left;
  const y = from.top + from.height / 2 - box.top;
  return `${String(x)}px ${String(y)}px`;
}

function enter(spring: SpringName, atZero: number | undefined, reduced: boolean, origin?: string): TransitionConfig {
  const motion = springMotion(spring, reduced);
  if (motion.type !== 'spring') {
    return { duration: modeTokens(reduced).duration.base, easing: easeCurve('enter'), css: opacity };
  }
  const { easing, durationMs } = springCurve(motion);
  return { duration: durationMs, easing, css: scaledFade(atZero, origin) };
}

function exit(atZero: number | undefined, reduced: boolean, origin?: string): TransitionConfig {
  const css = reduced ? opacity : scaledFade(atZero, origin);
  return { duration: modeTokens(reduced).duration.fast, easing: easeCurve('exit'), css };
}

/** Both layers run on `ease-enter`, so their opacities sum to 1 at every frame (5.7.3, MOT-008). */
export function crossFade(_node: Element, { preference = motionPreference() }: RoleParams = {}): RoleTransition {
  return () => ({
    duration: modeTokens(preference.reduced).duration.base,
    easing: easeCurve('enter'),
    css: opacity,
  });
}

export function fade(_node: Element, { preference = motionPreference() }: RoleParams = {}): RoleTransition {
  return ({ direction } = {}) => {
    const { duration } = modeTokens(preference.reduced);
    return direction === 'out'
      ? { duration: duration.fast, easing: easeCurve('exit'), css: opacity }
      : { duration: duration.base, easing: easeCurve('enter'), css: opacity };
  };
}

export function popover(node: Element, { preference = motionPreference(), from }: PopoverParams = {}): RoleTransition {
  const atZero = firstDirection<number | undefined>();
  return ({ direction } = {}) => {
    const origin = from === undefined ? undefined : originAt(from, node);
    return direction === 'out'
      ? exit(atZero(() => roleScale.popoverExit), preference.reduced, origin)
      : enter('snappy', atZero(() => roleScale.popoverEnter), preference.reduced, origin);
  };
}

/** The role's blur is left out: MOT-006 allows only transform and opacity to animate. */
export function modal(_node: Element, { preference = motionPreference() }: RoleParams = {}): RoleTransition {
  const atZero = firstDirection<number | undefined>();
  return ({ direction } = {}) =>
    direction === 'out'
      ? exit(atZero(() => roleScale.modal), preference.reduced)
      : enter('default', atZero(() => roleScale.modal), preference.reduced);
}

export function listRow(
  _node: Element,
  { preference = motionPreference(), index = 0 }: ListRowParams = {},
): RoleTransition {
  const atZero = firstDirection<number | undefined>();
  return ({ direction } = {}) => {
    if (direction === 'out') return exit(atZero(() => undefined), preference.reduced);
    const delay = staggerDelayMs(index, preference.reduced);
    return { ...enter('move', atZero(() => roleScale.listEnter), preference.reduced), delay };
  };
}
