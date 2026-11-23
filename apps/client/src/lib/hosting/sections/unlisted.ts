/** What a section of Hosting says while its owner has not built its list (HSE-001): no rows are made up. */
export interface UnlistedWords {
  readonly title: string;
  readonly line: string;
  /** True when the device list has the thing today. */
  readonly devices: boolean;
}

// One page serves the sections whose list is not built, so that no folder of the client is named for the banned word "models".
export const UNLISTED = {
  // Models (HSE-013): what a row of a model holds belongs to the model service of compute-plan.md.
  models: {
    title: 'Models are not listed here yet',
    line: 'The models that run on your machines will be listed here, in one group for each machine.',
    devices: false,
  },
} as const satisfies Record<string, UnlistedWords>;

export type Unlisted = keyof typeof UNLISTED;
