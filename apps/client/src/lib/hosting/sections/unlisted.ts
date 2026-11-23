/** What a section of Hosting says while its owner has not built its list (HSE-001): no rows are made up. */
export interface UnlistedWords {
  readonly title: string;
  readonly line: string;
  /** True when the device list has the thing today. */
  readonly devices: boolean;
}

// One page serves these two until their owners build their lists. Models has its page (`[section=copies]`, task L1-7).
export const UNLISTED = {
  // Storage (HSE-015): the volumes of each place belong to the hosted applications plan.
  storage: {
    title: 'Volumes are not listed here yet',
    line: "The volumes of each machine will be listed here, with the application that uses each one. Until then, a machine's volumes are on the Containers tab of its device page.",
    devices: true,
  },
  // Machines (HSE-016): what a machine reports and how it joins belong to the OS stream (spare-computer-os.md).
  machines: {
    title: 'Machines are not listed here yet',
    line: 'The machines that can host, and their room for more, will be listed here. Until then, each machine is a device on the Devices page.',
    devices: true,
  },
} as const satisfies Record<string, UnlistedWords>;

export type Unlisted = keyof typeof UNLISTED;
