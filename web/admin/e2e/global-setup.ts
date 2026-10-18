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
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { ensureCli, mintKey, runCli } from './harness/cli';
import { freePort, startProxy } from './harness/proxy';
import { prepareWorkDir, seedData, seedSystem, storeRoot } from './harness/seed';
import { APP_ROOT, DIST_ROOT, stateFile, writeHarness } from './harness/state';

const ADMIN_EMAIL = 'e2e-admin@example.com';
const ADMIN_NAME = 'E2E Admin';
const ADMIN_PASSWORD = 'e2e-password';
const HEALTH_TIMEOUT_MS = 30_000;
const HEALTH_INTERVAL_MS = 100;

const skip = (name: string): boolean => process.env[name] === '1';

const buildApp = (): void => {
  if (skip('OMEKA_E2E_SKIP_BUILD')) {
    console.log('e2e harness: skipping the SPA build (OMEKA_E2E_SKIP_BUILD=1)');
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
  const workDir = mkdtempSync(join(tmpdir(), 'omeka-e2e-work-'));
  prepareWorkDir(workDir);
  const dbPath = join(workDir, 'db.sqlite');

  buildApp();
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

  // `omeka serve` cannot report an ephemeral port, so it is told to listen on one
  // the probe just released. The origin then binds its own port and takes the
  // server's URL as its forwarding target.
  const serverPort = await freePort();
  const serverUrl = `http://127.0.0.1:${String(serverPort)}`;
  const proxy = await startProxy({
    distRoot: DIST_ROOT,
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
    ],
    { cwd: workDir, stdio: ['ignore', 'pipe', 'pipe'] },
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
  };

  try {
    await waitForHealth(serverUrl);
    const seeded = await seedData(proxy.url, cli, workDir, dbPath);
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
        `  origin     ${proxy.url} (serves ${DIST_ROOT}, /files and /s)`,
        `  port       ${serverUrl}`,
        `  work dir   ${workDir}`,
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
