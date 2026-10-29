/**
 * The harness for one `npm run e2e` run.
 *
 * Build the SPA, install a temporary database, mint an API key, start the port on
 * a free loopback port with the harness origin as its base URL, seed an item set,
 * items, media and a site through that origin, then hand the run over. The
 * returned teardown stops both servers and deletes the temporary directory.
 *
 * Nothing here binds a fixed port and nothing reads the coordinator's proxy.
 */
import { execFileSync, spawn } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { archiveRoot, writeArchive } from './harness/archive';
import { ensureCli, mintKey, runCli } from './harness/cli';
import { freePort, startProxy } from './harness/proxy';
import { seedData, seedSystem, storeRoot } from './harness/seed';
import { writeAdminSession } from './harness/session';
import { APP_ROOT, DIST_ROOT, ensureStateDir, sessionStateFile, stateFile, writeHarness } from './harness/state';

const ADMIN_EMAIL = 'e2e-admin@example.com';
const ADMIN_NAME = 'E2E Admin';
const ADMIN_PASSWORD = 'e2e-password';
const HEALTH_TIMEOUT_MS = 30_000;
const HEALTH_INTERVAL_MS = 100;

const skip = (name: string): boolean => process.env[name] === '1';

const buildApp = (): void => {
  if (skip('ROMEKA_E2E_SKIP_BUILD')) {
    console.log('e2e harness: skipping the SPA build (ROMEKA_E2E_SKIP_BUILD=1)');
    return;
  }
  execFileSync('npm', ['run', 'build'], { cwd: APP_ROOT, stdio: 'inherit' });
  if (!existsSync(join(DIST_ROOT, 'index.html'))) {
    throw new Error(`the build wrote no ${join(DIST_ROOT, 'index.html')}`);
  }
};

const waitForHealth = async (serverUrl: string): Promise<void> => {
  const deadline = Date.now() + HEALTH_TIMEOUT_MS;
  for (;;) {
    try {
      const response = await fetch(`${serverUrl}/health`);
      if (response.ok) {
        return;
      }
    } catch {
      // The listener is not up yet.
    }
    if (Date.now() > deadline) {
      throw new Error(`${serverUrl}/health did not answer within ${String(HEALTH_TIMEOUT_MS)}ms`);
    }
    await new Promise<void>((done) => {
      setTimeout(done, HEALTH_INTERVAL_MS);
    });
  }
};

export default async function globalSetup(): Promise<() => Promise<void>> {
  const workDir = mkdtempSync(join(tmpdir(), 'romeka-e2e-work-'));
  writeArchive(workDir);
  const dbPath = join(workDir, 'db.sqlite');

  buildApp();
  // The origin serves a copy of the build, not `dist` itself. A build started
  // while the run is live empties `dist` before it writes it again, and every
  // page load in that gap answered 404 (a spec failed on "not found").
  const servedRoot = join(workDir, 'dist');
  cpSync(DIST_ROOT, servedRoot, { recursive: true });
  const cli = ensureCli();
  runCli(
    cli,
    [
      'install',
      '-d',
      dbPath,
      '--email',
      ADMIN_EMAIL,
      '--name',
      ADMIN_NAME,
      '--password',
      ADMIN_PASSWORD,
    ],
    workDir,
  );
  const key = mintKey(cli, dbPath, ADMIN_EMAIL, workDir);

  // The asset and job rows go in before the port opens the database, so the
  // direct insert cannot race the server's connection.
  const system = seedSystem(workDir, dbPath, ADMIN_EMAIL);

  // `romeka serve` cannot report an ephemeral port, so it is told to listen on one
  // the probe just released. The origin then binds its own port and takes the
  // server's URL as its forwarding target.
  const serverPort = await freePort();
  const serverUrl = `http://127.0.0.1:${String(serverPort)}`;
  const proxy = await startProxy({
    distRoot: servedRoot,
    storeRoot: storeRoot(workDir),
    target: serverUrl,
    identity: key.identity,
    credential: key.credential,
  });

  const server = spawn(
    cli,
    [
      'serve',
      '-d',
      dbPath,
      '--host',
      '127.0.0.1',
      '--port',
      String(serverPort),
      '--base-url',
      proxy.url,
      '--store',
      storeRoot(workDir),
    ],
    // The archive root turns the Import page on. The server starts each import
    // job as its own `romeka job run` process with the same database and store.
    {
      cwd: workDir,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, ROMEKA_ARCHIVE_ROOT: archiveRoot(workDir) },
    },
  );
  let serverLog = '';
  server.stdout.on('data', (chunk: Buffer) => {
    serverLog += chunk.toString();
  });
  server.stderr.on('data', (chunk: Buffer) => {
    serverLog += chunk.toString();
  });

  const stopServer = async (): Promise<void> => {
    if (server.exitCode !== null || server.signalCode !== null) {
      return;
    }
    const stopped = new Promise<void>((done) => {
      server.once('close', () => {
        done();
      });
    });
    server.kill('SIGTERM');
    const timer = setTimeout(() => {
      server.kill('SIGKILL');
    }, 5_000);
    await stopped;
    clearTimeout(timer);
  };

  const teardown = async (): Promise<void> => {
    await proxy.close();
    await stopServer();
    rmSync(workDir, { recursive: true, force: true });
    rmSync(stateFile(), { force: true });
    rmSync(sessionStateFile(), { force: true });
  };

  try {
    await waitForHealth(serverUrl);
    const seeded = await seedData(proxy.url);
    ensureStateDir();
    proxy.shareSession(await writeAdminSession(proxy.url, ADMIN_EMAIL, ADMIN_PASSWORD));
    writeHarness({
      baseUrl: proxy.url,
      serverUrl,
      dbPath,
      storeRoot: storeRoot(workDir),
      workDir,
      keyIdentity: key.identity,
      keyCredential: key.credential,
      assetId: system.assetId,
      jobId: system.jobId,
      adminName: ADMIN_NAME,
      adminEmail: ADMIN_EMAIL,
      adminPassword: ADMIN_PASSWORD,
    });
    console.log(
      [
        'e2e harness ready',
        `  origin     ${proxy.url} (serves ${servedRoot}, a copy of ${DIST_ROOT}, /files and /s)`,
        `  port       ${serverUrl}`,
        `  work dir   ${workDir}`,
        `  archive    ${archiveRoot(workDir)}`,
        `  seeded     item set ${String(seeded.itemSetId)}, items ${seeded.itemIds.join(', ')}`,
        `             media ${String(seeded.mediaId)}, site ${String(seeded.siteId)}`,
        `             asset ${String(system.assetId)}, job ${String(system.jobId)}`,
      ].join('\n'),
    );
  } catch (error) {
    await teardown();
    throw new Error(
      `e2e harness setup failed: ${error instanceof Error ? error.message : String(error)}\n${serverLog}`,
      { cause: error },
    );
  }
  return teardown;
}
