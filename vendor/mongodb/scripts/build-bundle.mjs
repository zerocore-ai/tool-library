import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

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

async function main() {
  const toolDir = path.resolve(new URL('..', import.meta.url).pathname);
  const upstreamDir = path.join(toolDir, 'upstream');

  if (!(await pathExists(upstreamDir))) {
    throw new Error(`Missing upstream submodule at ${upstreamDir} (did you init submodules?)`);
  }

  const srcEsmDist = path.join(upstreamDir, 'dist', 'esm');
  const dstEsmDist = path.join(toolDir, 'server', 'mongodb-mcp-server', 'dist', 'esm');

  // 1) Build upstream
  // Avoid running upstream prepare hooks (husky) by ignoring scripts and building explicitly.
  await run('pnpm', ['install', '--frozen-lockfile', '--ignore-scripts'], { cwd: upstreamDir });
  await run('pnpm', ['run', 'build'], { cwd: upstreamDir });

  if (!(await pathExists(path.join(srcEsmDist, 'index.js')))) {
    throw new Error(`Expected build output not found at ${path.join(srcEsmDist, 'index.js')}`);
  }

  // 2) Vendor dist/esm into the MCPB bundle layout
  await fs.mkdir(path.dirname(dstEsmDist), { recursive: true });
  await fs.rm(dstEsmDist, { recursive: true, force: true });
  await fs.cp(srcEsmDist, dstEsmDist, { recursive: true });

  // NOTE: tool-cli's packer follows symlinks before applying .mcpbignore rules.
  // pnpm's node_modules layout can contain symlink loops, which breaks packing.
  // Clean the upstream workspace's node_modules after building to keep `tool pack` working.
  await fs.rm(path.join(upstreamDir, 'node_modules'), { recursive: true, force: true });
  await fs.rm(path.join(upstreamDir, 'tests', 'browser', 'node_modules'), { recursive: true, force: true });

  // 3) Install runtime deps for the bundled server (unless skipped)
  if (process.env.MCPB_SKIP_RUNTIME_DEPS === '1') {
    console.log('\nNOTE: Skipping runtime dependency install (MCPB_SKIP_RUNTIME_DEPS=1).');
    console.log('OK: mongodb upstream dist prepared.');
    console.log(`- Vendored server dist: ${path.relative(toolDir, dstEsmDist)}`);
    return;
  }

  await run('npm', ['ci', '--omit=dev', '--omit=optional', '--no-audit', '--no-fund'], {
    cwd: toolDir,
  });

  console.log('\nOK: mongodb MCPB bundle prepared.');
  console.log(`- Vendored server dist: ${path.relative(toolDir, dstEsmDist)}`);
  console.log(`- Runtime deps: ${path.relative(toolDir, path.join(toolDir, 'node_modules'))}`);
}

main().catch((err) => {
  console.error(`\nERROR: ${err?.message ?? String(err)}`);
  process.exit(1);
});
