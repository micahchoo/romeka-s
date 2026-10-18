/**
 * Fetch stubbing for the page tests. A handler maps a request URL to a
 * `Response`, so a page runs through the real typed client and its parsers.
 * Nothing here opens a network connection.
 */
import { vi } from 'vitest';
import type { Mock } from 'vitest';
import { isRecord } from '../src/api/parse';

export const requestUrl = (input: RequestInfo | URL): URL => {
  if (typeof input === 'string') {
    return new URL(input, window.location.href);
  }
  if (input instanceof URL) {
    return input;
  }
  return new URL(input.url, window.location.href);
};

export const jsonResponse = (
  body: unknown,
  options: { readonly status?: number; readonly headers?: Record<string, string> } = {},
): Response =>
  new Response(JSON.stringify(body), {
    status: options.status ?? 200,
    headers: { 'Content-Type': 'application/json', ...options.headers },
  });

/** A JSON response with the `Omeka-S-Total-Results` listing header. */
export const listResponse = (
  body: unknown,
  total: number,
  headers: Record<string, string> = {},
): Response => jsonResponse(body, { headers: { 'Omeka-S-Total-Results': String(total), ...headers } });

/**
 * The 204 the auth routes answer with. A `Response` whose status is 204 may not
 * carry a body, so `jsonResponse` cannot build one.
 */
export const noContent = (): Response => new Response(null, { status: 204 });

export type Handler = (url: URL, init: RequestInit | undefined) => Response;

/** Route by pathname; anything else is a 404 with an upstream error body. */
export const route =
  (routes: Readonly<Record<string, Handler>>): Handler =>
  (url, init) => {
    const handler = routes[url.pathname];
    if (handler === undefined) {
      return jsonResponse(
        { errors: { error: `Unexpected request to ${url.pathname}` } },
        { status: 404 },
      );
    }
    return handler(url, init);
  };

export const installFetch = (handler: Handler): Mock<typeof fetch> => {
  const fetchMock = vi.fn<typeof fetch>((input, init) =>
    Promise.resolve(handler(requestUrl(input), init)),
  );
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
};

export const urlsFor = (fetchMock: Mock<typeof fetch>, pathname: string): readonly URL[] =>
  fetchMock.mock.calls
    .map(([input]) => requestUrl(input))
    .filter((url) => url.pathname === pathname);

export interface CapturedRequest {
  readonly url: URL;
  readonly init: RequestInit | undefined;
}

/** Every call to one pathname with its request init, in call order. */
export const requestsFor = (
  fetchMock: Mock<typeof fetch>,
  pathname: string,
): readonly CapturedRequest[] =>
  fetchMock.mock.calls.flatMap(([input, init]) => {
    const url = requestUrl(input);
    return url.pathname === pathname ? [{ url, init }] : [];
  });

/** The JSON object a captured request sent. */
export const requestBody = (request: CapturedRequest): Record<string, unknown> => {
  const body = request.init?.body;
  if (typeof body !== 'string') {
    throw new Error('expected a JSON string request body');
  }
  const value: unknown = JSON.parse(body);
  if (!isRecord(value)) {
    throw new Error('expected a JSON object request body');
  }
  return value;
};

export const first = <T,>(values: readonly T[]): T => {
  const value = values[0];
  if (value === undefined) {
    throw new Error('expected at least one value');
  }
  return value;
};

export const linkHeader = (rels: ReadonlyArray<readonly [string, number]>): string =>
  rels
    .map(([rel, page]) => `<http://localhost/api/x?page=${String(page)}>; rel="${rel}"`)
    .join(', ');
