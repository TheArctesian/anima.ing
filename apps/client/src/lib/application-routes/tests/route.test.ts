import { expect, it } from 'vitest';
import { dnsRecords, makePublicRefusal, routeChecks } from '../checks';
import { bytes, certificateStatus, count, filterCounts, filtered, isNotLinux, proofStatus, serving, summary, target, traffic } from '../route';
import { DEVICE_NAMES, NOT_LINUX_ERROR, NOW, ROUTES, route } from './routes.fixture';

it('H21: the proof says what the name waits for on a hosted hub and on a hub that a person hosts', () => {
  expect(proofStatus(route({ proof: 'WAITING' })).label).toBe('Waits for TXT record');
  expect(proofStatus(route({ proof: 'WAITING', proofRecordName: null })).label).toBe('Waits for the name');
  expect(proofStatus(route({ proof: 'LOST', failedReads: 3 }))).toEqual({ label: 'Name lost', tone: 'down', detail: 'TXT record not found, 3 reads.' });
  expect(proofStatus(route({})).tone).toBe('up');
});

it('H21: a certificate that ends in fewer than 30 days needs the operator; a stream has none', () => {
  expect(certificateStatus(route({ certificateNotAfter: '2026-11-30T13:00:00Z' }), NOW, 'en-GB')).toMatchObject({ label: 'Certificate ends in 9 days', tone: 'degraded' });
  expect(certificateStatus(route({}), NOW, 'en-GB')).toMatchObject({ label: 'Certificate to 18 Feb 2027', tone: 'up' });
  expect(certificateStatus(route({ certificateNotAfter: null }), NOW).label).toBe('No certificate yet');
  expect(certificateStatus(route({ certificateNotAfter: '2026-11-01T00:00:00Z' }), NOW).tone).toBe('down');
  expect(certificateStatus(route({ kind: 'TCP_STREAM' }), NOW).detail).toBe('A stream has no TLS of the hub.');
});

it('H40: the target is the device and the port, the port of the hub of a stream, or the address of a redirect', () => {
  expect(target(route({}), DEVICE_NAMES)).toEqual({ head: 'On atlas', detail: 'port 80' });
  expect(target(route({ kind: 'TCP_STREAM', port: 993, listenPort: 10993 }), DEVICE_NAMES).detail).toBe('port 993, from port 10993 of the hub');
  expect(target(route({ kind: 'REDIRECT', port: null, redirectTo: 'https://git.example.com/x' }), DEVICE_NAMES)).toEqual({ head: 'Redirect to', detail: 'git.example.com/x' });
  expect(target(route({ deviceId: 'gone' }), DEVICE_NAMES).head).toBe('On a device that is not listed');
});

it('H21: the state of a route follows the step that it waits for', () => {
  expect(serving(route({ waitsFor: null })).label).toBe('Serving, private');
  expect(serving(route({ waitsFor: null, access: 'PUBLIC' })).label).toBe('Serving, public');
  expect(serving(route({})).label).toBe('Waits for consent at the machine');
  expect(serving(route({ waitsFor: 'APPLICATION_RECORD' })).label).toBe('Waits for the record of the application');
  expect(serving(route({ waitsFor: 'CERTIFICATE' })).label).toBe('Waits for a certificate');
  expect(serving(route({ waitsFor: 'PROOF', proof: 'WAITING' })).label).toBe('Waits for TXT record');
});

it('H21: the filter keeps the routes that need the operator, the public ones and the private ones, in the order of their names', () => {
  expect(filterCounts(ROUTES, NOW)).toEqual({ ALL: 5, NEEDS_YOU: 3, PUBLIC: 2, PRIVATE: 3 });
  expect(filtered(ROUTES, 'NEEDS_YOU', NOW).map((each) => each.name)).toEqual(['media.example.org', 'tracks.example.com', 'wiki.example.com']);
  expect(filtered(ROUTES, 'PUBLIC', NOW).map((each) => each.name)).toEqual(['code.example.com', 'media.example.org']);
});

it('H21: the summary counts the routes that serve and gives the time of the last check', () => {
  expect(summary(ROUTES)).toMatch(/^0 serving, 5 waiting · checked /);
  expect(summary([route({ checkedAt: null, waitsFor: null })])).toBe('1 serving, 0 waiting · not checked yet');
});

it('H21: the traffic says why a route that waits passes nothing', () => {
  expect(traffic(route({ refused: 12 })).detail).toBe('12 refused');
  expect(traffic(route({})).head).toBe('Not serving');
  expect(traffic(route({ waitsFor: null, requests: 41_210, bytesOut: 3_100_000_000 }))).toEqual({ head: '41.2k requests', detail: '3.1 GB out' });
  expect([count(312), count(1_900), count(2_500_000), bytes(41_000), bytes(84_000_000), bytes(512)]).toEqual(['312', '1.9k', '2.5M', '41 KB', '84 MB', '512 B']);
});

it('H21: the checks are in the order of the hub; the record of the application follows from the step that the route waits for', () => {
  const checks = routeChecks(route({}), 'atlas', NOW);
  expect(checks.map((each) => [each.title, each.label])).toEqual([
    ['The name is proven', 'Proven'],
    ['Consent at the machine', 'Not given'],
    ['Record of the application', 'Not checked yet'],
    ['Certificate', 'Valid'],
  ]);
  expect(checks[1]?.detail).toContain('OD-HOST-43');
  expect(routeChecks(route({ waitsFor: 'APPLICATION_RECORD', machineConsent: true }), 'atlas', NOW)[2]).toMatchObject({ label: 'Waits', detail: expect.stringContaining('task H6') as unknown });
  expect(routeChecks(route({ waitsFor: null, machineConsent: true }), 'atlas', NOW)[2]?.label).toBe('Recorded');
  expect(routeChecks(route({ kind: 'TCP_STREAM' }), 'atlas', NOW).map((each) => each.title)).not.toContain('Certificate');
  expect(routeChecks(route({ proofRecordName: null }), 'atlas', NOW)[0]?.title).toBe('The name points at this hub');
});

it('H21: a hosted hub gives the TXT record; the address of the door and of a hub that a person hosts are not in the API', () => {
  expect(dnsRecords(route({}))).toEqual([
    { type: 'TXT', name: '_anima.vault.example.com', valueLabel: 'Value', value: 'anima-route=hub_1.token' },
    { type: 'CNAME', name: 'vault.example.com', valueLabel: 'Target', value: null },
  ]);
  expect(dnsRecords(route({ proofRecordName: null, proofRecordValue: null }))).toEqual([{ type: 'A', name: 'vault.example.com', valueLabel: 'Address', value: null }]);
});

it('OD-HOST-43: Make public is refused while the consent at the machine is not given', () => {
  expect(makePublicRefusal(route({}), 'atlas')).toMatch(/^Consent at atlas is not given\. Port 80 is open to the hub/);
  expect(makePublicRefusal(route({ machineConsent: true }), 'atlas')).toBe('The hub has no step that makes a route public yet.');
});

it('R-366: only the refusal of a hub that is not on Linux is that refusal', () => {
  expect(isNotLinux(NOT_LINUX_ERROR)).toBe(true);
  expect(isNotLinux({ code: 'UNAVAILABLE', message: 'The hub did not answer.' })).toBe(false);
});
