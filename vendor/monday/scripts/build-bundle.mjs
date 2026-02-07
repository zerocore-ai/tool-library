import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

function run(cmd, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, {
      stdio: 'inherit',
      ...options,
    });

    child.on('error', reject);
    child.on('exit', (code, signal) => {
      if (code === 0) return resolve();
      reject(
        new Error(
          `Command failed: ${cmd} ${args.join(' ')} (code=${code ?? 'null'}, signal=${
            signal ?? 'null'
          })`
        )
      );
    });
  });
}

async function pathExists(p) {
  try {
    await fs.access(p);
    return true;
  } catch {
    return false;
  }
}

async function ensureUpstreamSubmoduleReady({ toolDir, upstreamDir, env }) {
  const required = [path.join(upstreamDir, 'package.json'), path.join(upstreamDir, 'yarn.lock')];
  const missing = [];

  for (const filePath of required) {
    if (!(await pathExists(filePath))) missing.push(path.basename(filePath));
  }

  if (missing.length === 0) return;

  console.log(
    `\nNOTE: Upstream submodule checkout looks incomplete (missing: ${missing.join(', ')}).\n` +
      `Attempting to init/update the submodule...`
  );

  try {
    await run('git', ['submodule', 'sync', '--recursive'], { cwd: toolDir, env });
    await run('git', ['submodule', 'update', '--init', '--recursive', 'upstream'], {
      cwd: toolDir,
      env,
    });
  } catch (err) {
    throw new Error(
      `Failed to init/update upstream submodule.\n` +
        `- Expected files: ${required.map((p) => path.relative(toolDir, p)).join(', ')}\n` +
        `- Try: git submodule update --init --recursive vendor/monday/upstream\n` +
        `- Original error: ${err?.message ?? String(err)}`
    );
  }

  for (const filePath of required) {
    if (!(await pathExists(filePath))) {
      throw new Error(
        `Upstream submodule is still missing ${path.relative(toolDir, filePath)} after init/update.\n` +
          `Try: git submodule update --init --recursive vendor/monday/upstream`
      );
    }
  }
}

async function main() {
  const currentMajor = Number.parseInt(process.versions.node.split('.')[0], 10);
  if (!Number.isFinite(currentMajor)) {
    throw new Error(`Unable to parse Node version: ${process.version}`);
  }

  const toolDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

  // This repo can be installed/run with many Node versions, but building the bundle
  // must use Node 20/22 to avoid native dependency issues (e.g. isolated-vm).
  const isSupportedBuildNode = currentMajor === 20 || currentMajor === 22;

  // If tool-cli runs this script under an unsupported Node (common when global Node is 23+),
  // keep going but force child processes to use a supported Node if available.
  const preferredNodeBins = [
    process.env.MCPB_NODE_BIN,
    '/opt/homebrew/opt/node@20/bin/node',
    '/usr/local/opt/node@20/bin/node',
    '/opt/homebrew/opt/node@22/bin/node',
    '/usr/local/opt/node@22/bin/node',
  ].filter(Boolean);

  let nodeBinForBuild = process.execPath;
  if (!isSupportedBuildNode) {
    for (const candidate of preferredNodeBins) {
      if (await pathExists(candidate)) {
        nodeBinForBuild = candidate;
        break;
      }
    }
  }

  const nodeBinDirForBuild = path.dirname(nodeBinForBuild);
  const buildEnv = {
    ...process.env,
    PATH: `${nodeBinDirForBuild}:${process.env.PATH ?? ''}`,
  };

  if (!isSupportedBuildNode && nodeBinForBuild === process.execPath) {
    throw new Error(
      `Node.js ${process.version} is not supported for this bundle build, and no Node 20/22 was found.\n` +
        `Install Node 20/22 (recommended via Homebrew node@20) or set MCPB_NODE_BIN to a Node 20/22 binary path.\n` +
        `Example: MCPB_NODE_BIN=/opt/homebrew/opt/node@20/bin/node tool build`
    );
  }

  const upstreamDir = path.join(toolDir, 'upstream');

  const srcDist = path.join(upstreamDir, 'packages', 'monday-api-mcp', 'dist');
  const dstDist = path.join(toolDir, 'server', 'monday-api-mcp', 'dist');

  await ensureUpstreamSubmoduleReady({ toolDir, upstreamDir, env: buildEnv });

  // 1) Build upstream (monorepo)
  await run('yarn', ['install', '--frozen-lockfile'], { cwd: upstreamDir, env: buildEnv });
  await run('yarn', ['workspace', '@mondaydotcomorg/agent-toolkit', 'build'], {
    cwd: upstreamDir,
    env: buildEnv,
  });
  await run('yarn', ['workspace', '@mondaydotcomorg/monday-api-mcp', 'build'], {
    cwd: upstreamDir,
    env: buildEnv,
  });

  if (!(await pathExists(srcDist))) {
    throw new Error(`Expected build output not found at ${srcDist}`);
  }

  // 2) Vendor dist/ into the MCPB bundle layout
  await fs.mkdir(path.dirname(dstDist), { recursive: true });
  await fs.rm(dstDist, { recursive: true, force: true });
  await fs.cp(srcDist, dstDist, { recursive: true });

  // 3) Install runtime deps for the bundled server (unless skipped)
  if (process.env.MCPB_SKIP_RUNTIME_DEPS === '1') {
    console.log('\nNOTE: Skipping runtime dependency install (MCPB_SKIP_RUNTIME_DEPS=1).');
    console.log('OK: monday upstream dist prepared.');
    console.log(`- Vendored server dist: ${path.relative(toolDir, dstDist)}`);
    return;
  }

  await run('npm', ['ci', '--omit=dev', '--no-audit', '--no-fund'], { cwd: toolDir, env: buildEnv });

  console.log('\nOK: monday MCPB bundle prepared.');
  console.log(`- Vendored server dist: ${path.relative(toolDir, dstDist)}`);
  console.log(`- Runtime deps: ${path.relative(toolDir, path.join(toolDir, 'node_modules'))}`);
}

main().catch((err) => {
  console.error(`\nERROR: ${err?.message ?? String(err)}`);
  process.exit(1);
});
