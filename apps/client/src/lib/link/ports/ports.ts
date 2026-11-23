// The open port switches (HMD-049).

export type System = 'mac' | 'linux' | 'windows';

export interface PortRow {
  readonly port: number;
  readonly label: string;
  /** The preparation needed for a private connection (HMD-049, OS3-R12). */
  readonly needs: string;
  readonly on: boolean;
}

const NEEDS: Readonly<Record<System, Readonly<Record<number, string>>>> = {
  mac: {
    22: 'Sets up SSH automatically. Your system may ask for administrator approval.',
    5900: 'Needs Screen Sharing: System Settings, General, Sharing.',
  },
  linux: {
    22: 'Sets up SSH automatically. Your system may ask for administrator approval.',
    3389: 'Sets up a private RDP server automatically on supported Linux desktops. Your system may ask for administrator approval.',
    5900: 'Needs an existing VNC server that requires authentication before screen access. Anima cannot verify a login screen inside a VNC session.',
  },
  windows: {
    22: 'Sets up SSH automatically. Your system may ask for administrator approval.',
    5900: 'Needs an existing VNC server that requires authentication before screen access.',
    3389: 'Needs Remote Desktop: Settings, System, Remote Desktop.',
  },
};

const LABELS: Readonly<Record<number, string>> = {
  22: 'SSH (port 22)',
  5900: 'Screen sharing (port 5900)',
  3389: 'Remote Desktop (port 3389)',
};

export function systemOf(userAgent: string): System {
  if (/Windows/.test(userAgent)) return 'windows';
  if (/Macintosh|Mac OS X/.test(userAgent)) return 'mac';
  return 'linux';
}

export function portRows(system: System, open: readonly number[]): readonly PortRow[] {
  return Object.entries(NEEDS[system]).map(([port, needs]) => {
    const number = Number(port);
    return { port: number, label: LABELS[number] ?? `Port ${port}`, needs, on: open.includes(number) };
  });
}
