import { Buffer } from 'node:buffer';
import console from 'node:console';
import { sign } from 'node:crypto';
import process from 'node:process';
import { setTimeout as wait } from 'node:timers/promises';
import { URL, URLSearchParams } from 'node:url';

const origin = 'https://api.appstoreconnect.apple.com';
const publicInvite = 'https://testflight.apple.com/join/SwDsVg4Z';
const internal = process.env.GITHUB_REF_NAME === 'dev';
const mode = process.argv[2];

function jwt() {
  const { ASC_API_KEY_ID, ASC_API_ISSUER_ID, ASC_API_PRIVATE_KEY } = process.env;
  const encode = (value) => Buffer.from(JSON.stringify(value)).toString('base64url');
  const header = encode({ alg: 'ES256', kid: ASC_API_KEY_ID, typ: 'JWT' });
  const now = Math.floor(Date.now() / 1000);
  const payload = encode({ iss: ASC_API_ISSUER_ID, iat: now, exp: now + 600, aud: 'appstoreconnect-v1' });
  const message = `${header}.${payload}`;
  const signature = sign('sha256', Buffer.from(message), { key: ASC_API_PRIVATE_KEY, dsaEncoding: 'ieee-p1363' });
  return `${message}.${signature.toString('base64url')}`;
}

async function apple(method, endpoint, data) {
  const url = new URL(endpoint, origin);
  if (url.origin !== origin) throw new Error('Apple returned an unexpected pagination origin.');
  const response = await globalThis.fetch(url, {
    method,
    redirect: 'error',
    signal: globalThis.AbortSignal.timeout(30_000),
    headers: { Authorization: `Bearer ${jwt()}`, 'Content-Type': 'application/json' },
    ...(data === undefined ? {} : { body: JSON.stringify({ data }) }),
  });
  if (response.status === 204) return {};
  const result = await response.json();
  if (!response.ok) {
    const codes = (result.errors ?? []).map((error) => error.code).join(', ');
    throw new Error(`App Store Connect ${method} ${url.pathname} returned HTTP ${response.status} (${codes}).`);
  }
  return result;
}

async function list(endpoint) {
  const resources = [];
  for (let page = endpoint; page;) {
    const answer = await apple('GET', page);
    resources.push(...answer.data);
    page = answer.links?.next;
  }
  return resources;
}

function only(resources, name) {
  if (resources.length > 1) throw new Error(`App Store Connect returned multiple matches for ${name}.`);
  return resources[0];
}

function query(path, values) {
  return `${path}?${new URLSearchParams(values)}`;
}

function resource(type, id) {
  return { type, id };
}

async function groupsFor(app) {
  return list(`/v1/apps/${app}/betaGroups?limit=200`);
}

function groupFor(groups) {
  const testers = internal ? 'Anima Internal' : publicInvite;
  const group = only(groups.filter(({ attributes }) => internal
    ? attributes.isInternalGroup && attributes.name === testers
    : !attributes.isInternalGroup && attributes.publicLinkEnabled && attributes.publicLink === testers), testers);
  if (!group) throw new Error(`App Store Connect has no active TestFlight group for ${testers}.`);
  return group;
}

async function buildFor(app) {
  const endpoint = query('/v1/builds', {
    'filter[app]': app,
    'filter[version]': process.env.TESTFLIGHT_BUILD,
    'filter[preReleaseVersion.version]': process.env.TESTFLIGHT_VERSION,
    'filter[preReleaseVersion.platform]': 'IOS',
  });
  return only(await list(endpoint), 'iOS build number and version');
}

async function processedBuild(app) {
  for (let check = 0; check < 90; check += 1) {
    const build = await buildFor(app);
    if (build?.attributes.processingState === 'VALID') return build;
    if (build && build.attributes.processingState !== 'PROCESSING') {
      throw new Error(`The uploaded build is ${build.attributes.processingState}.`);
    }
    await wait(30_000);
  }
  throw new Error('App Store Connect is still processing the build. Rerun the job to resume.');
}

async function prepareExternal(build) {
  const beta = (await apple('GET', `/v1/builds/${build}/buildBetaDetail`)).data;
  const localizations = await list(`/v1/builds/${build}/betaBuildLocalizations`);
  if (!localizations.some((locale) => locale.attributes.locale === 'en-US')) {
    await apple('POST', '/v1/betaBuildLocalizations', {
      type: 'betaBuildLocalizations',
      attributes: { locale: 'en-US', whatsNew: 'Test sign-in and device management on iPhone and iPad.' },
      relationships: { build: { data: resource('builds', build) } },
    });
  }
  await apple('PATCH', `/v1/buildBetaDetails/${beta.id}`, {
    ...resource('buildBetaDetails', beta.id),
    attributes: { autoNotifyEnabled: true },
  });
}

async function submitExternal(build) {
  const beta = (await apple('GET', `/v1/builds/${build}/buildBetaDetail`)).data;
  const state = beta.attributes.externalBuildState;
  if (['WAITING_FOR_BETA_REVIEW', 'IN_BETA_REVIEW', 'BETA_APPROVED', 'READY_FOR_BETA_TESTING', 'IN_BETA_TESTING'].includes(state)) {
    console.log(`External TestFlight state: ${state}.`);
    return;
  }
  if (state !== 'READY_FOR_BETA_SUBMISSION') {
    throw new Error(`External beta review is blocked by ${state}. Check export compliance and beta review details in App Store Connect.`);
  }
  await apple('POST', '/v1/betaAppReviewSubmissions', {
    type: 'betaAppReviewSubmissions',
    relationships: { build: { data: resource('builds', build) } },
  });
  console.log('Submitted for TestFlight Beta App Review.');
}

async function retainInternalBuilds(app, membership, assigned, current) {
  const members = new Set([...assigned.map(({ id }) => id), current]);
  const builds = await list(query('/v1/builds', {
    'filter[app]': app,
    'filter[preReleaseVersion.platform]': 'IOS',
    'filter[expired]': 'false',
    limit: '200',
  }));
  const active = builds.filter(({ id, attributes }) => members.has(id)
    && attributes.buildAudienceType === 'INTERNAL_ONLY'
    && attributes.processingState === 'VALID'
    && attributes.expired === false
    && Number.isFinite(Date.parse(attributes.uploadedDate)));
  active.sort((a, b) => Date.parse(b.attributes.uploadedDate) - Date.parse(a.attributes.uploadedDate)
    || Number(b.id === current) - Number(a.id === current));
  if (active[0]?.id !== current) return;
  const older = active.slice(2);
  if (older.length === 0) return;
  await apple('DELETE', membership, older.map(({ id }) => resource('builds', id)));
  console.log(`Removed ${older.length} older builds from Anima Internal.`);
}

async function distribute(app, group) {
  const build = await processedBuild(app);
  const expected = internal ? 'INTERNAL_ONLY' : 'APP_STORE_ELIGIBLE';
  if (build.attributes.buildAudienceType !== expected || build.attributes.expired) {
    throw new Error('App Store Connect reports an incompatible or expired build.');
  }
  if (internal) {
    const { data: beta } = await apple('GET', `/v1/builds/${build.id}/buildBetaDetail`);
    if (!['READY_FOR_BETA_TESTING', 'IN_BETA_TESTING'].includes(beta.attributes.internalBuildState)) {
      throw new Error(`Internal testing is blocked by ${beta.attributes.internalBuildState}. Check the build in App Store Connect.`);
    }
  } else await prepareExternal(build.id);

  const membership = `/v1/betaGroups/${group.id}/relationships/builds`;
  const assigned = await list(`${membership}?limit=200`);
  if (!assigned.some((each) => each.id === build.id)) {
    await apple('POST', membership, [resource('builds', build.id)]);
  }
  console.log(`Build ${build.attributes.version} assigned to ${group.attributes.name}.`);
  if (internal) await retainInternalBuilds(app, membership, assigned, build.id);
  else await submitExternal(build.id);
}

async function main() {
  if (!['dev', 'main'].includes(process.env.GITHUB_REF_NAME) || !['present', 'distribute'].includes(mode)) {
    throw new Error('Specify a branch-scoped TestFlight operation.');
  }
  const apps = await list('/v1/apps?filter%5BbundleId%5D=ing.anima.mobile');
  const app = only(apps, 'ing.anima.mobile')?.id;
  if (!app) throw new Error('The API key has no access to the Anima iOS app.');
  const groups = await groupsFor(app);
  if (!internal && groups.some(({ attributes }) => attributes.name === 'Anima Internal' && attributes.hasAccessToAllBuilds)) {
    throw new Error('Turn off automatic distribution for Anima Internal before uploading on main.');
  }
  const group = groupFor(groups);
  if (internal && group.attributes.hasAccessToAllBuilds) {
    throw new Error('Turn off automatic distribution for Anima Internal to manage its build list.');
  }
  if (mode === 'present') process.exitCode = await buildFor(app) ? 0 : 3;
  else await distribute(app, group);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
