import { expect, it } from 'vitest';
import { Favourites } from './favourites.svelte';
import { pickerCatalogue } from '../picker/fixture';

function memory(start: Record<string, string> = {}): Storage {
  const items = new Map(Object.entries(start));
  return { getItem: (key: string) => items.get(key) ?? null, setItem: (key: string, value: string) => items.set(key, value) } as unknown as Storage;
}

it('marks a favourite model in the catalogue and keeps it on this device for the next start', () => {
  const storage = memory();
  const model = pickerCatalogue.models[1];
  if (model === undefined) throw new Error('no model');
  new Favourites(storage).set(model, true);
  const marked = new Favourites(storage).apply(pickerCatalogue).models.filter((each) => each.favourite);
  expect(marked.map((each) => each.id)).toEqual([model.id]);
});

it('forgets a removed favourite, and a broken store means none', () => {
  const storage = memory({ 'anima.code.favouriteModels': 'not json' });
  const favourites = new Favourites(storage);
  expect(favourites.apply(pickerCatalogue).models.some((each) => each.favourite)).toBe(false);
  const model = pickerCatalogue.models[0];
  if (model === undefined) throw new Error('no model');
  favourites.set(model, true);
  favourites.set(model, false);
  expect(new Favourites(storage).apply(pickerCatalogue).models.some((each) => each.favourite)).toBe(false);
});
