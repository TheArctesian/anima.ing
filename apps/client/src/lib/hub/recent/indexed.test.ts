import { IDBFactory } from 'fake-indexeddb';
import { describe, expect, it } from 'vitest';
import type { Copy } from './copy';
import { indexedCopyStore } from './indexed';

const copy = (key: string, at: number, text = `"${key}"`): Copy => ({ key, at, text });

function deleted(factory: IDBFactory): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = factory.deleteDatabase('anima-recent');
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error ?? new Error('not deleted'));
  });
}

describe('the copy store in IndexedDB (FND-007)', () => {
  it('gives no copy for a key that it never kept', async () => {
    expect(await indexedCopyStore(new IDBFactory()).read('a')).toBeUndefined();
  });

  it('gives the copy that it kept, and a later copy with the same key takes its place', async () => {
    const store = indexedCopyStore(new IDBFactory());
    await store.write(copy('a', 1, '"first"'), 10);
    expect(await store.read('a')).toEqual(copy('a', 1, '"first"'));
    await store.write(copy('a', 2, '"second"'), 10);
    expect(await store.read('a')).toEqual(copy('a', 2, '"second"'));
  });

  it('keeps the copy for the next start of the application', async () => {
    const factory = new IDBFactory();
    await indexedCopyStore(factory).write(copy('a', 1), 10);
    expect(await indexedCopyStore(factory).read('a')).toEqual(copy('a', 1));
  });

  it('drops the oldest copies above the limit, by their time and not by the order of the writes', async () => {
    const store = indexedCopyStore(new IDBFactory());
    await store.write(copy('new', 30), 2);
    await store.write(copy('old', 10), 2);
    await store.write(copy('middle', 20), 2);
    expect(await store.read('old')).toBeUndefined();
    expect(await store.read('middle')).toEqual(copy('middle', 20));
    expect(await store.read('new')).toEqual(copy('new', 30));
  });
});

describe('what leaves the copy store in IndexedDB (FND-007)', () => {
  it('removes one copy and leaves the others', async () => {
    const store = indexedCopyStore(new IDBFactory());
    await store.write(copy('a', 1), 10);
    await store.write(copy('b', 2), 10);
    await store.remove('a');
    expect(await store.read('a')).toBeUndefined();
    expect(await store.read('b')).toEqual(copy('b', 2));
  });

  it('removes every copy at sign-out, also for the next start of the application', async () => {
    const factory = new IDBFactory();
    const store = indexedCopyStore(factory);
    await store.write(copy('a', 1), 10);
    await store.write(copy('b', 2), 10);
    await store.clear();
    expect(await store.read('a')).toBeUndefined();
    expect(await indexedCopyStore(factory).read('b')).toBeUndefined();
  });

  it('works again after the browser deleted its database', async () => {
    const factory = new IDBFactory();
    const store = indexedCopyStore(factory);
    await store.write(copy('a', 1), 10);
    await deleted(factory);
    expect(await store.read('a')).toBeUndefined();
    await store.write(copy('b', 2), 10);
    expect(await store.read('b')).toEqual(copy('b', 2));
  });

  it('rejects when IndexedDB refuses to open, so that the caller has no copy', async () => {
    const refused = new Error('refused');
    const factory = {
      open: () => {
        throw refused;
      },
    } as unknown as IDBFactory;
    const store = indexedCopyStore(factory);
    await expect(store.read('a')).rejects.toBe(refused);
    await expect(store.write(copy('a', 1), 10)).rejects.toBe(refused);
  });
});
