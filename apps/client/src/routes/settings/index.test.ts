import { expect, it } from 'vitest';
import { indexOf } from './index';

const DESKTOP = { keyStore: true, stayAwake: true, desktopRows: true, mcpPermissions: true };

it('R-222: the index has VPN for a hub with a VPN, and leaves it out for a hub that Anima.ing hosts', () => {
  expect(indexOf({ ...DESKTOP, vpn: true }).map((entry) => entry.title)).toContain('VPN');
  expect(indexOf({ ...DESKTOP, vpn: false }).map((entry) => entry.title)).not.toContain('VPN');
});

it('R-224: the index is in three groups, and each entry names its group', () => {
  const entries = indexOf({ ...DESKTOP, vpn: true });
  expect(entries.map((entry) => entry.title)).toEqual(['Account', 'Hub', 'Routes', 'Notifications', 'Default check settings', 'Data retention', 'Stay awake', 'Appearance', 'Updates', 'MCP', 'Integrations', 'API keys', 'VPN']);
  expect([...new Set(entries.map((entry) => entry.group))]).toEqual(['Hub and account', 'This computer', 'Connections']);
  expect(entries.find((entry) => entry.title === 'Appearance')?.group).toBe('This computer');
});

it('H21: Routes is in the group Hub and account, after Hub, on each platform', () => {
  const browser = indexOf({ keyStore: false, stayAwake: false, desktopRows: false, mcpPermissions: false, vpn: false });
  expect(browser.slice(0, 2).map((entry) => entry.title)).toEqual(['Account', 'Routes']);
  expect(indexOf({ ...DESKTOP, vpn: true }).find((entry) => entry.title === 'Routes')?.group).toBe('Hub and account');
});
