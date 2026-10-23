/**
 * Structure's "Mark an example" on a name whose changing part is glued to the
 * object's number (`BHC006_GMT_AF_010a.jpg`, view a of object 010): the
 * screen shows the number and the letter as two parts, and marking the
 * letter as any letters saves the mark and the pattern it writes.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { patternOf } from '../src/pages/import/example';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { installFetch } from './stub';

const NAME = 'BHC006_GMT_AF_010a.jpg';

afterEach(() => {
  window.location.hash = '';
});

describe('a changing part glued to the number', () => {
  it('shows 010 and a as two parts, and marking a as any letters saves that mark', async () => {
    const server = simpleServer({ ...scenario('bhc', 9), examples: [`Artifacts/${NAME}`, 'Artifacts/BHC006_GMT_AF_010b.jpg'] });
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Mark an example' }, { timeout: 5000 }));
    fireEvent.click(await screen.findByRole('radio', { name: NAME }, { timeout: 5000 }));
    const parts = await screen.findByRole('group', { name: `Parts of the name ${NAME}` });
    expect(within(parts).getAllByRole('button').map((button) => button.textContent)).toEqual(['BHC006', 'GMT', 'AF', '010', 'a']);
    expect(parts.textContent).toBe(NAME);

    fireEvent.click(within(parts).getByRole('button', { name: 'a' }));
    fireEvent.click(screen.getByRole('radio', { name: /^Any letters, as in “a”/u }));
    const use = await screen.findByRole('button', { name: 'Use this' }, { timeout: 5000 });
    await waitFor(() => {
      expect(use).toBeEnabled();
    });
    fireEvent.click(use);
    const mark = { kind: 'letters', text: 'a', sep: '' } as const;
    await waitFor(() => {
      const plan = server.saved[server.saved.length - 1] ?? {};
      expect(asRecord(plan['example'], [])['marks']).toEqual([mark]);
      expect(asRecord(plan['grouping'], [])['pattern']).toBe(patternOf([mark]));
    }, { timeout: 5000 });
  });
});
