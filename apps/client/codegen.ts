import { checkTypes, clientLayout, writeTypes } from './src/lib/hub/generation.ts';

const layout = clientLayout(import.meta.dirname);

if (process.argv.includes('--check')) {
  const stale = await checkTypes(layout);
  for (const { path, reason } of stale) console.error(`${path}: ${reason}; run \`bun run --cwd apps/client graphql\` (API-008)`);
  process.exit(stale.length > 0 ? 1 : 0);
}

for (const path of await writeTypes(layout)) console.log(`wrote ${path}`);
