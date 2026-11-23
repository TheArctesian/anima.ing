import type { ParamMatcher } from '@sveltejs/kit';

/** Hosting > Models (HSE-013): its folder is not named for the banned word "models", so this matcher gives it the address. */
export const match = ((param: string): param is 'models' => param === 'models') satisfies ParamMatcher;
