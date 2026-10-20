/**
 * A stand-in for the routes the run screen calls, answering the P4-api
 * fixtures, so the screen runs through the real client and its parsers.
 * `reads` are the answers of successive `GET …/imports/{id}`; the last one
 * repeats.
 */
import { asRecord } from '../src/api/parse';
import { loadFixture } from './loadFixture';
import { jsonResponse, requestBody, route } from './stub';
import type { CapturedRequest, Handler } from './stub';

export const fixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);

export const conflict = (name: string, members: Record<string, unknown> = {}): Response =>
  jsonResponse({ ...fixture(name), ...members }, { status: 409 });

/** An undo report fixture as the import route answers it: the undo job's row with the import's plan. */
export const reported = (name: string): Record<string, unknown> => {
  const { plan, ranPlan } = fixture('import-done.json');
  return { ...fixture(name), plan, ranPlan };
};

export const pathOf = (id: number): string => `/api/romeka/imports/${String(id)}`;

/** The check requests among `requests`: the status line over the screen asks for other sections. */
export const checks = (requests: readonly CapturedRequest[]): readonly CapturedRequest[] =>
  requests.filter((request) => {
    const include = requestBody(request)['include'];
    return Array.isArray(include) && include.length === 1 && include[0] === 'check';
  });

export interface RunRoutes {
  readonly reads: readonly Record<string, unknown>[];
  /** The fixture the check section comes from. */
  readonly check?: string;
  /** Successive answers of `POST …/run`; the last repeats. */
  readonly run?: readonly (() => Response)[];
  readonly stop?: () => Response;
  readonly undo?: () => Response;
  readonly undoPreview?: () => Response;
  readonly left?: () => Response;
}

export const runRoutes = (id: number, routes: RunRoutes): Handler => {
  let reads = 0;
  let runs = 0;
  const path = pathOf(id);
  const pick = <T,>(answers: readonly T[], index: number): T => {
    const answer = answers[Math.min(index, answers.length - 1)];
    if (answer === undefined) {
      throw new Error('a route with no answer');
    }
    return answer;
  };
  return route({
    [path]: () => {
      const answer = pick(routes.reads, reads);
      reads += 1;
      return jsonResponse(answer);
    },
    [`${path}/preview`]: (_url, init) => {
      const include = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), [])['include'];
      const check = fixture(routes.check ?? 'preview-check.json');
      const answer: Record<string, unknown> = { sources: check['sources'], pattern: check['pattern'] };
      for (const section of Array.isArray(include) ? include : []) {
        answer[String(section)] = section === 'check' ? check['check'] : fixture(`preview-${String(section)}.json`)[String(section)];
      }
      return jsonResponse(answer);
    },
    [`${path}/run`]: () => {
      const answer = pick(routes.run ?? [() => jsonResponse(fixture('run.json'))], runs);
      runs += 1;
      return answer();
    },
    [`${path}/stop`]: routes.stop ?? (() => jsonResponse(fixture('stop.json'), { status: 202 })),
    [`${path}/undo`]: routes.undo ?? (() => jsonResponse(fixture('undo.json'))),
    [`${path}/undo/preview`]: routes.undoPreview ?? (() => jsonResponse(fixture('undo-preview-kept.json'))),
    [`${path}/left.csv`]:
      routes.left ??
      (() => new Response('kind,source,path,record,#reason\nfile,s1,_scratch/notes.txt,,not-media\n', { headers: { 'Content-Type': 'text/csv' } })),
  });
};
