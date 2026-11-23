import type { MenuEntry } from '$lib/code/kit/menu';
import { PATCH } from './scenes';

export const PATCHES = `${PATCH}diff --git a/src/routes/sign-in/back.test.ts b/src/routes/sign-in/back.test.ts
new file mode 100644
index 0000000..5c1e9d2
--- /dev/null
+++ b/src/routes/sign-in/back.test.ts
@@ -0,0 +1,8 @@
+import { expect, it } from 'vitest';
+import { back } from './back';
+
+it('keeps a path of this site and refuses a different site', () => {
+  expect(back('/code')).toBe('/code');
+  expect(back('//elsewhere.example')).toBe('/');
+  expect(back(null)).toBe('/');
+});
diff --git a/src/routes/sign-in/first.ts b/src/routes/sign-in/first.ts
deleted file mode 100644
index 77aa001..0000000
--- a/src/routes/sign-in/first.ts
+++ /dev/null
@@ -1,3 +0,0 @@
-export function first(): string {
-  return '/';
-}
diff --git a/docs/login.md b/docs/sign-in.md
similarity index 90%
rename from docs/login.md
rename to docs/sign-in.md
index 1a2b3c4..5d6e7f8 100644
--- a/docs/login.md
+++ b/docs/sign-in.md
@@ -1,3 +1,3 @@
 # Sign-in
 
-After the sign-in the person goes to the first page.
+After the sign-in the person goes back to the page that the link named.
`;

export const SCOPES: readonly MenuEntry[] = [
  { kind: 'choice', id: 'branch', label: 'Changes' },
  { kind: 'choice', id: 'unstaged', label: 'Uncommitted' },
  { kind: 'choice', id: 'turn', label: 'Latest turn' },
  {
    kind: 'choice',
    id: 'turns',
    label: 'Turn',
    items: [
      { kind: 'choice', id: 'turn:3', label: 'Turn 3', detail: '14:02' },
      { kind: 'choice', id: 'turn:2', label: 'Turn 2', detail: '13:40' },
    ],
  },
];

export const PROJECT_PATHS: readonly string[] = [
  'README.md',
  'package.json',
  'docs/sign-in.md',
  'node_modules/',
  'src/app.d.ts',
  'src/routes/+layout.svelte',
  'src/routes/sign-in/+page.svelte',
  'src/routes/sign-in/back.ts',
  'src/routes/sign-in/back.test.ts',
  'static/mark.svg',
];
