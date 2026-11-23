import type { ParamMatcher } from '@sveltejs/kit';
import { UNLISTED, type Unlisted } from '$lib/hosting/sections/unlisted';

/** The sections of Hosting whose list their owner has not built yet; each other name under /hosting is not found. */
export const match = ((param: string): param is Unlisted => Object.hasOwn(UNLISTED, param)) satisfies ParamMatcher;
