import { parseDeviceId } from './device-id';
import type { PageLoad } from './$types';

/** The ID is parsed at the route edge (D-14). */
export const load: PageLoad = ({ params }) => ({ id: parseDeviceId(params.id) });
