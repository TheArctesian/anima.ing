import { describe, expect, it } from 'vitest';
import { MALFORMED, needsNewerHub, parseResponse, UNSERVED } from './error';

function failure(error: unknown): unknown {
  return { data: null, errors: [error] };
}

describe('parseResponse', () => {
  it('returns the data of an answer without errors', () => {
    expect(parseResponse({ data: { devices: [] } })).toEqual({ ok: true, value: { devices: [] } });
  });

  it.each(['UNAUTHENTICATED', 'FORBIDDEN', 'NOT_FOUND', 'CONFLICT', 'UNAVAILABLE', 'INTERNAL'])('K7: extensions.code %s becomes that typed error', (code) => {
    const body = failure({ message: 'Plain sentence.', extensions: { code } });
    expect(parseResponse(body)).toEqual({ ok: false, error: { code, message: 'Plain sentence.' } });
  });

  it('K7: VALIDATION carries its field list', () => {
    const fields = [{ field: 'name', message: 'Type a name.' }];
    const body = failure({ message: 'Check the form.', extensions: { code: 'VALIDATION', fields } });
    expect(parseResponse(body)).toEqual({ ok: false, error: { code: 'VALIDATION', message: 'Check the form.', fields } });
  });

  it('K7: VALIDATION without a field list has no fields', () => {
    const body = failure({ message: 'Check the form.', extensions: { code: 'VALIDATION' } });
    expect(parseResponse(body)).toEqual({ ok: false, error: { code: 'VALIDATION', message: 'Check the form.', fields: [] } });
  });

  it.each(['The query is too deep.', 'Unknown type "OtherInput"'])('an unrelated uncoded error stays INTERNAL: %s', (message) => {
    const body = { errors: [{ message }] };
    expect(parseResponse(body)).toEqual({ ok: false, error: { code: 'INTERNAL', message } });
  });

  it.each([
    'Unknown field "projectGroups" on type "Query".',
    'Invalid value for argument "input", unknown field "includeEmpty" of type "ProjectGroupImportInput"',
  ])('a field the hub does not know is unserved: %s', (message) => {
    const answer = parseResponse({ errors: [{ message }] });
    expect(answer).toEqual({ ok: false, error: UNSERVED });
    expect(!answer.ok && needsNewerHub(answer.error)).toBe(true);
  });

  it('an unknown code is INTERNAL', () => {
    const body = failure({ message: 'Odd.', extensions: { code: 'TEAPOT' } });
    expect(parseResponse(body)).toEqual({ ok: false, error: { code: 'INTERNAL', message: 'Odd.' } });
  });

  it('errors win over partial data', () => {
    const body = { data: { device: { id: 'x' } }, errors: [{ message: 'Gone.', extensions: { code: 'NOT_FOUND' } }] };
    expect(parseResponse(body)).toEqual({ ok: false, error: { code: 'NOT_FOUND', message: 'Gone.' } });
  });

  it.each([
    ['null', null],
    ['a string', 'Bad Gateway'],
    ['an array', [1, 2]],
    ['an empty object', {}],
    ['null data and no errors', { data: null }],
    ['an empty error list', { errors: [] }],
    ['errors that are not a list', { errors: 'broken' }],
    ['an error without a message', { errors: [{ extensions: { code: 'FORBIDDEN' } }] }],
    ['a field list that is not a list', failure({ message: 'x', extensions: { code: 'VALIDATION', fields: 'name' } })],
    ['a field without a message', failure({ message: 'x', extensions: { code: 'VALIDATION', fields: [{ field: 'name' }] } })],
  ])('%s is malformed', (_label, body) => {
    expect(parseResponse(body)).toEqual({ ok: false, error: MALFORMED });
  });
});
