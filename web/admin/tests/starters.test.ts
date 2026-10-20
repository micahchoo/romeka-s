/**
 * The server's starter table through the client (`src/api/starters.ts`), and
 * the Spanish text that must follow it: a name or label with no Spanish entry
 * reads English, so a server rename shows up here first.
 */
import { describe, expect, it } from 'vitest';
import { ApiClient } from '../src/api/client';
import { ParseError } from '../src/api/parse';
import { getStarters, parseStarters } from '../src/api/starters';
import { SPANISH_LABELS, SPANISH_NAMES } from '../src/pages/item-types/starter-text';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestsFor } from './stub';

describe('the starter table', () => {
  it('is read from the server route and keeps the server order', async () => {
    const fetchMock = installFetch(() => jsonResponse(loadFixture('starters.json')));
    const table = await getStarters(new ApiClient({ baseUrl: '' }));
    expect(requestsFor(fetchMock, '/api/romeka/starters')).toHaveLength(1);
    expect(table.starters.map((starter) => starter.id)).toEqual([
      'letter',
      'photograph',
      'postcard',
      'diary',
      'person',
      'other',
    ]);
    expect(table.other.name).toBe('Physical object');
    expect(table.starters[0]?.fields[2]).toEqual({
      term: 'dcterms:creator',
      required: false,
      label: 'Sender',
      dataTypes: ['literal'],
    });
  });

  it('refuses a table with no starter for an unknown kind', () => {
    const letterOnly = [loadFixture('starters.json')].flat().slice(0, 1);
    expect(() => parseStarters(letterOnly)).toThrow(ParseError);
  });

  it('has Spanish for every name and label the server sends', () => {
    const { starters } = parseStarters(loadFixture('starters.json'));
    expect(starters.filter((starter) => SPANISH_NAMES[starter.nameKey] === undefined)).toEqual([]);
    const labels = new Set(starters.flatMap((starter) => starter.fields.map((entry) => entry.label)));
    expect([...labels].filter((label) => SPANISH_LABELS[label] === undefined)).toEqual([]);
  });
});
