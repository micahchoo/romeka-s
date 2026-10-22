/**
 * Advanced's pattern editor on a plan of several folders: BHC006 after the
 * first Structure click (`proposal-bhc006.json`). A typed pattern is counted
 * the way Simple counts a marked example: the proposal of the plan with the
 * typed pattern and `own` on the picked folder, whose `own` node is that
 * folder as typed (`proposal-bhc006-own.json`: Artifacts, 328 items, 696 of
 * 696 images). The sample name comes from the picked folder's own files.
 * No count of the typed pattern is asked of the whole plan.
 */
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const PATH = '/api/romeka/imports/1';

const importFixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);

const PROPOSAL = importFixture('proposal-bhc006.json');
const OWN = importFixture('proposal-bhc006-own.json');
const OWN_ARTIFACTS = { edit: 'own', scope: { source: 's1', folder: 'Artifacts' } };
/** The pattern of `proposal-bhc006-own.json`'s written Artifacts (`bhc006_tests::MARKED_PATTERN`). */
const TYPED = String.raw`^(?:[^/]*/)*(?<identifier>[^/]*?\d|[^/]*?[^/.][\p{L}\p{M}]*)(?:(?<part>[\p{L}\p{M}]+))?(?:\.[^./]*)?$`;

interface Asked {
  readonly include: readonly string[];
  readonly plan: Record<string, unknown>;
  readonly entries: Record<string, unknown> | null;
}

/** The draft holds the proposal's structure; each preview answers from its own fixture, a proposal by the edits it was sent. */
const server = (): { readonly asked: Asked[]; readonly saved: Record<string, unknown>[] } => {
  const detail = importFixture('import-draft.json');
  let plan: Record<string, unknown> = { ...asRecord(detail['plan'], []), structure: PROPOSAL['structure'] };
  let revision = 0;
  const asked: Asked[] = [];
  const saved: Record<string, unknown>[] = [];
  installFetch(
    route({
      [PATH]: (_url, init) => {
        if (init?.method === 'PUT') {
          plan = asRecord(asRecord(JSON.parse(typeof init.body === 'string' ? init.body : '{}'), [])['plan'], []);
          saved.push(plan);
          revision += 1;
        }
        return jsonResponse({ ...detail, plan, revision });
      },
      [`${PATH}/preview`]: (_url, init) => {
        const body = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
        const include = Array.isArray(body['include']) ? body['include'].map(String) : [];
        const sent = isRecord(body['plan']) ? body['plan'] : plan;
        asked.push({ include, plan: sent, entries: isRecord(body['entries']) ? body['entries'] : null });
        const owned = JSON.stringify(sent['edits']) === JSON.stringify([OWN_ARTIFACTS]);
        const answer: Record<string, unknown> = { sources: importFixture('preview-check.json')['sources'], pattern: { groups: ['identifier', 'part'] } };
        for (const section of include) {
          answer[section] = section === 'proposal' ? (owned ? OWN : PROPOSAL) : importFixture(`preview-${section}.json`)[section];
        }
        return jsonResponse(answer);
      },
      '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
    }),
  );
  return { asked, saved };
};

afterEach(() => {
  cleanup();
  window.location.hash = '';
});

describe('the pattern editor of one folder', () => {
  it("counts the picked folder's files, not the plan's", async () => {
    const { asked, saved } = server();
    window.location.hash = '#/import/1/advanced/files/{"series":"Artifacts"}';
    render(<ImportPage param="1" rest={['1', 'advanced', 'files', '{"series":"Artifacts"}']} />);
    fireEvent.change(await screen.findByLabelText('Regex', undefined, { timeout: 5000 }), { target: { value: TYPED } });
    expect(await screen.findByText('696 of 696 files fit. The files that fit make 328 items.', undefined, { timeout: 5000 })).toBeInTheDocument();

    const trial = asked.filter((one) => one.include.includes('proposal')).at(-1);
    const levels: unknown = asRecord(trial?.plan['structure'], [])['levels'];
    const artifacts = (Array.isArray(levels) ? levels : []).filter(isRecord).find((level) => level['id'] === 'Artifacts');
    expect(artifacts?.['grouping']).toEqual({ pattern: TYPED, caseFold: false });
    expect(trial?.plan['edits']).toEqual([OWN_ARTIFACTS]);
    expect(trial?.entries?.['folder']).toBe('Artifacts');
    const typed = asked.filter((one) => JSON.stringify(one.plan).includes(JSON.stringify(TYPED)));
    expect(typed.length).toBeGreaterThan(0);
    expect(typed.filter((one) => one.include.includes('histogram') || (one.entries !== null && one.entries['folder'] !== 'Artifacts'))).toEqual([]);
    expect(document.querySelector('.adv-parts-name')).toHaveTextContent('BHC_006_AF_001_back.jpg');
    expect(document.querySelector('.adv-parts-name')).not.toHaveTextContent('Artifacts/');
    expect(saved).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Use this pattern' }));
    await waitFor(() => {
      expect(saved).toHaveLength(1);
    });
    expect(saved[0]?.['edits']).toEqual([OWN_ARTIFACTS]);
  }, 15_000);
});
