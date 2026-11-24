/**
 * Which changed file is a test file (t3-ui-copy-2026-11-21/05, Section 1.13): a test or a mock, by its name or by a
 * folder of its path. "Hide test files" takes these from the view only.
 */
const FOLDERS = new Set([
  '__tests__', '__test__', 'cypress', 'e2e', 'evals', 'spec', 'specs', 'test', 'tests',
  '__fixtures__', '__mocks__', '__snapshots__', 'fakes', 'fixture', 'fixtures', 'mock', 'mocks', 'stubs', 'testdata',
]);
const INFIX = /\.(test|spec|e2e|eval|stories|mocks?|fixtures?|stubs?|fakes?)\.[^.]+$/i;
const SUFFIX = /_(test|spec)\.[^.]+$/i;
const SETUP = /^(test_.*\.py|conftest\.py|(vitest|jest)\.setup\..+)$/i;
const CLASS = /(Test|Tests|Spec)\.(java|kt|kts|scala|groovy|cs|php|swift|m|mm)$/;
const MOCK_START = /^(mocks?|fixtures?|stubs?|fakes?)[._-]/i;

export function isTestFile(path: string): boolean {
  const parts = path.split('/');
  const name = parts.pop() ?? '';
  if (parts.some((folder) => FOLDERS.has(folder))) return true;
  return INFIX.test(name) || SUFFIX.test(name) || SETUP.test(name) || CLASS.test(name) || name.endsWith('.snap') || MOCK_START.test(name);
}
