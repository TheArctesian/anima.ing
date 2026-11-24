import type { ParamMatcher } from '@sveltejs/kit';
import { UNLISTED, type Unlisted } from '$lib/hosting/sections/unlisted';

export const match = ((param: string): param is Unlisted => Object.hasOwn(UNLISTED, param)) satisfies ParamMatcher;
