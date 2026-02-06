import fs from 'node:fs';
import path from 'node:path';

const DEFAULT_RUNS_ON_BY_TARGET = {
  'darwin-x86_64': 'macos-15-intel',
  'darwin-arm64': 'macos-15',
  'linux-x86_64': 'ubuntu-24.04',
  'linux-arm64': 'ubuntu-24.04-arm',
  'win32-x86_64': 'windows-2022',
  'win32-arm64': 'windows-11-arm',
  universal: 'ubuntu-24.04',
};

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function setOutput(key, value) {
  const outPath = process.env.GITHUB_OUTPUT;
  if (!outPath) {
    throw new Error('GITHUB_OUTPUT is not set');
  }
  fs.appendFileSync(outPath, `${key}=${value}\n`);
}

function fail(message) {
  console.error(`ERROR: ${message}`);
  process.exit(1);
}

function main() {
  const repoRoot = process.cwd();
  const configPath =
    process.env.VENDOR_RELEASE_CONFIG ||
    process.env.EXTERNAL_RELEASE_CONFIG ||
    path.join(repoRoot, '.github', 'vendor-release.json');

  if (!fs.existsSync(configPath)) {
    fail(`Missing config file: ${configPath}`);
  }

  const config = readJson(configPath);
  const vendors =
    config?.vendors && typeof config.vendors === 'object'
      ? config.vendors
      : config?.externals && typeof config.externals === 'object'
        ? config.externals
        : null;

  if (!vendors) {
    fail(`Invalid config: expected { vendors: { ... } } in ${configPath}`);
  }

  const runsOnByTarget =
    config?.runners && typeof config.runners === 'object'
      ? { ...DEFAULT_RUNS_ON_BY_TARGET, ...config.runners }
      : DEFAULT_RUNS_ON_BY_TARGET;

  const buildInclude = [];
  const distInclude = [];

  for (const [vendor, opts] of Object.entries(vendors)) {
    const targets = Array.isArray(opts?.targets) ? opts.targets : [];
    if (targets.length === 0) {
      fail(`Config vendors.${vendor}.targets must be a non-empty array`);
    }

    const needsDist = Boolean(opts?.needs_dist);
    const npmCiArgs = typeof opts?.npm_ci_args === 'string' ? opts.npm_ci_args : '';

    if (needsDist) {
      distInclude.push({
        vendor,
        runs_on: 'ubuntu-24.04',
      });
    }

    for (const target of targets) {
      const runsOn = runsOnByTarget[target];
      if (!runsOn) {
        fail(
          `Unknown target '${target}' for vendor '${vendor}'. Add a runner mapping in .github/vendor-release.json (runners.${target}) or update DEFAULT_RUNS_ON_BY_TARGET.`
        );
      }

      buildInclude.push({
        vendor,
        target,
        runs_on: runsOn,
        needs_dist: needsDist ? 'true' : 'false',
        npm_ci_args: npmCiArgs,
      });
    }
  }

  const buildMatrix = JSON.stringify({ include: buildInclude });
  const distMatrix = JSON.stringify({ include: distInclude });

  setOutput('build_matrix', buildMatrix);
  setOutput('dist_matrix', distMatrix);
  setOutput('dist_enabled', distInclude.length > 0 ? 'true' : 'false');
}

main();
