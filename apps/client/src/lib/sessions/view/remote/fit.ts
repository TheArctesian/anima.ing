import type { RemoteProtocol } from '../../api';
import type { Size } from '../display';

/**
 * RDP is sized to the pane in device pixels (RA-004); VNC shrinks to fit but never enlarges, so a small screen stays sharp.
 * The scale is a static transform, never animated (D-5, MAT-008).
 */
export function displayScale(protocol: RemoteProtocol, remote: Size, pane: Size, devicePixelRatio: number): number {
  if (protocol === 'RDP') return Number.isFinite(devicePixelRatio) && devicePixelRatio > 0 ? 1 / devicePixelRatio : 1;
  const known = (size: Size) => Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0;
  if (!known(remote) || !known(pane)) return 1;
  return Math.min(1, pane.width / remote.width, pane.height / remote.height);
}
