/**
 * The harness origin: one origin that serves the built SPA, the media file store
 * and the port's own routes.
 *
 * The SPA asks for `/api` on the origin it was loaded from
 * (`src/api/client.ts::defaultBaseUrl`), and `romeka serve` renders its JSON-LD
 * links from the base URL it was started with. Both point at this origin, so the
 * browser sees one origin for the app, the API, the files and the public site.
 * That is the arrangement a reverse proxy would produce in production.
 *
 * Every `/api` request leaves here with `key_identity` and `key_credential`
 * appended, the pair `omeka_api::auth::authenticate` reads. Restricted reads and
 * every write need it, and upstream presents the key exactly this way. This file
 * is the harness's own proxy; the suite does not depend on the coordinator's.
 */
import { createReadStream, existsSync, statSync } from 'node:fs';
import { createServer, request as httpRequest } from 'node:http';
import type { IncomingMessage, ServerResponse } from 'node:http';
import { extname, resolve, sep } from 'node:path';

const MIME_TYPES: Readonly<Record<string, string>> = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.ico': 'image/x-icon',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.txt': 'text/plain; charset=utf-8',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
};

export interface ProxyOptions {
  /** The built SPA: the run's copy of `web/admin/dist`. */
  readonly distRoot: string;
  /** The file store root, served under `/files`. */
  readonly storeRoot: string;
  /** Origin of the running `romeka serve`. */
  readonly target: string;
  readonly identity: string;
  readonly credential: string;
}

export interface RunningProxy {
  readonly url: string;
  readonly close: () => Promise<void>;
}

/**
 * Paths the port answers: the API, its context, health, the public site, its
 * Logout form, the published term lists and vocabularies, the IIIF viewer,
 * manifests and images, the Web Annotation Protocol routes, and upstream's
 * fallback thumbnails under `/application/asset/` (which `deploy/Caddyfile`
 * also sends to the port).
 */
const portPaths = (pathname: string): boolean =>
  pathname === '/api' ||
  pathname.startsWith('/api/') ||
  pathname === '/api-context' ||
  pathname === '/health' ||
  pathname === '/logout' ||
  pathname === '/s' ||
  pathname.startsWith('/s/') ||
  pathname === '/vocab' ||
  pathname.startsWith('/vocab/') ||
  pathname === '/iiif-viewer' ||
  pathname.startsWith('/iiif/') ||
  pathname === '/annotations' ||
  pathname.startsWith('/annotations/') ||
  pathname.startsWith('/application/');

const apiPaths = (pathname: string): boolean =>
  pathname === '/api' || pathname.startsWith('/api/');

/** The file a pathname names inside a static root, or `null` when it escapes. */
const fileFor = (root: string, pathname: string): string | null => {
  const file = resolve(root, `.${pathname}`);
  if (file !== root && !file.startsWith(root + sep)) {
    return null;
  }
  return file;
};

const sendFile = (res: ServerResponse, file: string): void => {
  res.writeHead(200, {
    'content-type': MIME_TYPES[extname(file)] ?? 'application/octet-stream',
  });
  createReadStream(file).pipe(res);
};

/**
 * Serve a file from `root`. A miss falls back to `fallback` only when the
 * pathname has no extension, so a missing stylesheet is a 404 and never the SPA
 * shell.
 */
const serveStatic = (
  root: string,
  pathname: string,
  res: ServerResponse,
  fallback?: string,
): boolean => {
  const file = fileFor(root, pathname);
  if (file !== null && existsSync(file) && statSync(file).isFile()) {
    sendFile(res, file);
    return true;
  }
  if (fallback !== undefined && extname(pathname) === '') {
    const shell = fileFor(root, fallback);
    if (shell !== null && existsSync(shell)) {
      sendFile(res, shell);
      return true;
    }
  }
  return false;
};

const notFound = (res: ServerResponse): void => {
  res.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
  res.end('not found');
};

const forward = (
  url: URL,
  req: IncomingMessage,
  res: ServerResponse,
  options: ProxyOptions,
): void => {
  if (apiPaths(url.pathname)) {
    url.searchParams.set('key_identity', options.identity);
    url.searchParams.set('key_credential', options.credential);
  }
  const headers = { ...req.headers };
  delete headers.host;
  const upstream = httpRequest(
    new URL(`${url.pathname}${url.search}`, options.target),
    { method: req.method ?? 'GET', headers },
    (answer) => {
      res.writeHead(answer.statusCode ?? 502, answer.headers);
      answer.pipe(res);
    },
  );
  upstream.on('error', (error: Error) => {
    res.writeHead(502, { 'content-type': 'text/plain; charset=utf-8' });
    res.end(`upstream error: ${error.message}`);
  });
  req.pipe(upstream);
};

/**
 * Bind `port` on the loopback interface (0 picks a free one) and answer the four
 * request families the app makes: the store, the port's routes, the bundle and
 * the theme's assets behind it.
 */
export const startProxy = async (options: ProxyOptions, port = 0): Promise<RunningProxy> => {
  const server = createServer((req: IncomingMessage, res: ServerResponse) => {
    const url = new URL(req.url ?? '/', 'http://harness.local');
    if (url.pathname.startsWith('/files/')) {
      if (!serveStatic(options.storeRoot, url.pathname.slice('/files'.length), res)) {
        notFound(res);
      }
      return;
    }
    if (portPaths(url.pathname)) {
      forward(url, req, res, options);
      return;
    }
    if (serveStatic(options.distRoot, url.pathname, res, '/index.html')) {
      return;
    }
    // The SPA's bundle and the theme's files share the `/assets` prefix, as
    // `deploy/Caddyfile` arranges: a file the bundle ships wins, and every
    // other path under the prefix is the port's theme route.
    if (url.pathname.startsWith('/assets/')) {
      forward(url, req, res, options);
      return;
    }
    notFound(res);
  });
  await new Promise<void>((done, fail) => {
    server.once('error', (error: Error) => {
      fail(error);
    });
    server.listen(port, '127.0.0.1', () => {
      done();
    });
  });
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('harness proxy: no bound port');
  }
  return {
    url: `http://127.0.0.1:${String(address.port)}`,
    close: async (): Promise<void> => {
      server.closeAllConnections();
      await new Promise<void>((done) => {
        server.close(() => {
          done();
        });
      });
    },
  };
};

/** A free loopback port. The CLI cannot bind port 0 and report the result. */
export const freePort = async (): Promise<number> => {
  const probe = createServer();
  await new Promise<void>((done, fail) => {
    probe.once('error', (error: Error) => {
      fail(error);
    });
    probe.listen(0, '127.0.0.1', () => {
      done();
    });
  });
  const address = probe.address();
  if (address === null || typeof address === 'string') {
    throw new Error('free port probe: no bound port');
  }
  const port = address.port;
  await new Promise<void>((done) => {
    probe.close(() => {
      done();
    });
  });
  return port;
};
