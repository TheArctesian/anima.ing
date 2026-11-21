import { isRedirect } from '@sveltejs/kit';
import { expect, it } from 'vitest';
import { load } from '../../routes/+page';

it('NAV-004: / redirects to /devices', () => {
  let thrown: unknown;
  try {
    load();
  } catch (error) {
    thrown = error;
  }
  expect(isRedirect(thrown)).toBe(true);
  expect(thrown).toMatchObject({ status: 307, location: '/devices' });
});

it('Anima Code: an old link to /threads goes on to /code', async () => {
  const { load: threads } = await import('../../routes/threads/+page');
  let thrown: unknown;
  try {
    threads();
  } catch (error) {
    thrown = error;
  }
  expect(isRedirect(thrown)).toBe(true);
  expect(thrown).toMatchObject({ status: 308, location: '/code' });
});
