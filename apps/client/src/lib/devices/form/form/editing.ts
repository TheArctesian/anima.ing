import type { DeviceFormValues } from '../values';

type Writable<T> = { -readonly [Key in keyof T]: T[Key] extends readonly (infer Entry)[] ? Entry[] : Writable<T[Key]> };

export type EditedValues = Writable<DeviceFormValues>;
