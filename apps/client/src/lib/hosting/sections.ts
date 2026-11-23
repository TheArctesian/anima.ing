import type { IconName } from '$lib/icons/icons';

/** The address of a section of Hosting: each section has its own, so a link and the back button open it (HSE-001). */
export type HostingPath = '/hosting' | '/hosting/catalogue' | '/hosting/models' | '/hosting/storage' | '/hosting/routes' | '/hosting/machines';

/** One section of Hosting: its address, its name, its icon and the line under its heading. */
export interface HostingSection {
  readonly path: HostingPath;
  readonly title: string;
  readonly icon: IconName;
  readonly description: string;
  /** A page below the section before it in the list: the Catalogue below Applications (HSE-004). */
  readonly below?: true;
  /** The body has no heading of its own: its action is in the page header (Routes, mockup routes Frame 1). */
  readonly bare?: true;
}

/** The sections of Hosting, in the order of HSE-001 (R-426, R-428). */
export const HOSTING_SECTIONS: readonly HostingSection[] = [
  { path: '/hosting', title: 'Applications', icon: 'carbon:grid', description: 'The applications and containers of each machine.' },
  { path: '/hosting/catalogue', title: 'Catalogue', icon: 'carbon:catalog', description: 'The applications that ship with Anima.', below: true },
  { path: '/hosting/models', title: 'Models', icon: 'carbon:chip', description: 'The models that run on your machines.' },
  { path: '/hosting/storage', title: 'Storage', icon: 'carbon:vmdk-disk', description: 'The volumes of each machine.' },
  { path: '/hosting/routes', title: 'Routes', icon: 'carbon:server-dns', description: 'Names of yours that the hub sends to your applications.', bare: true },
  { path: '/hosting/machines', title: 'Machines', icon: 'carbon:bare-metal-server', description: 'The places where things can run.' },
];

/** The section whose address is `pathname`, or one below it; Applications for any other address of Hosting. */
export function sectionOf(pathname: string): HostingSection {
  const path = pathname.replace(/\/+$/, '') || '/';
  const [applications] = HOSTING_SECTIONS;
  return HOSTING_SECTIONS.find((section) => section.path !== '/hosting' && (path === section.path || path.startsWith(`${section.path}/`))) ?? (applications as HostingSection);
}

/** The section named `path`. */
export function section(path: HostingPath): HostingSection {
  return HOSTING_SECTIONS.find((each) => each.path === path) as HostingSection;
}
