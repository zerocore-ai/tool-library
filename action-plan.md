# tool-action: GitHub Action for MCPB Bundles

## Overview

Create a GitHub Action (`zerocore-ai/tool-action`) that provides building blocks for packing and publishing MCPB bundles. Users define their own matrix/workflow structure; the action provides reusable steps.

## Repository Structure

```
zerocore-ai/tool-action/
├── setup/
│   └── action.yml          # Install tool-cli
├── pack/
│   └── action.yml          # Pack bundle for a platform
├── publish/
│   └── action.yml          # Publish multi-platform bundles
├── validate/
│   └── action.yml          # Validate manifest (optional, nice-to-have)
├── LICENSE
└── README.md
```

---

## Action Specifications

### 1. `tool-action/setup`

**Purpose:** Install tool-cli binary.

**Inputs:**
| Input | Default | Description |
|-------|---------|-------------|
| `version` | `latest` | tool-cli version to install |
| `fallback-to-source` | `true` | Build from source if prebuilt unavailable |

**Outputs:**
| Output | Description |
|--------|-------------|
| `version` | Installed tool-cli version string |

**Logic:**
1. Try to install prebuilt binary via install script:
   ```bash
   curl -fsSL "https://raw.githubusercontent.com/zerocore-ai/tool-cli/main/install.sh" | sh -s -- \
     --version="${version}" \
     --prefix="$HOME/.local" \
     --force \
     --no-modify-path
   echo "$HOME/.local/bin" >> "$GITHUB_PATH"
   ```
2. If prebuilt fails and `fallback-to-source` is true:
   - Setup Rust toolchain (`dtolnay/rust-toolchain@stable`)
   - `cargo install --git "https://github.com/zerocore-ai/tool-cli" --locked --force`
   - Add `$HOME/.cargo/bin` to PATH
3. Verify with `tool --version` and set output

---

### 2. `tool-action/pack`

**Purpose:** Pack an MCPB bundle, optionally for a specific platform target.

**Inputs:**
| Input | Default | Description |
|-------|---------|-------------|
| `target` | (none) | Platform target suffix (e.g., `darwin-arm64`). If omitted, creates universal bundle. |
| `output-dir` | `dist` | Directory to place the bundle |
| `checksum` | `true` | Generate `.sha256` checksum file |
| `working-directory` | `.` | Directory containing manifest.json |

**Outputs:**
| Output | Description |
|--------|-------------|
| `bundle-path` | Path to the created bundle file |
| `bundle-name` | Filename of the bundle |
| `checksum-path` | Path to checksum file (if generated) |

**Logic:**
1. Change to `working-directory`
2. Remove any existing bundles: `rm -f ./*.mcpb ./*.mcpbx`
3. Run `tool pack`
4. Find the created bundle (expect exactly one `.mcpb` or `.mcpbx` file)
5. Rename with target suffix if `target` is provided:
   ```
   my-tool-0.1.0.mcpb → my-tool-0.1.0-darwin-arm64.mcpb
   ```
6. Move to `output-dir`
7. If `checksum` is true, generate SHA256:
   ```bash
   sha256sum bundle.mcpb > bundle.mcpb.sha256
   # or use node/openssl for cross-platform compatibility
   ```
8. Set outputs

---

### 3. `tool-action/publish`

**Purpose:** Publish multi-platform bundles to tool.store registry.

**Inputs:**
| Input | Default | Description |
|-------|---------|-------------|
| `bundles` | `dist/*.mcpb` | Glob pattern for bundle files |
| `dry-run` | `false` | Validate without uploading |
| `working-directory` | `.` | Directory containing manifest.json |

**Outputs:**
| Output | Description |
|--------|-------------|
| `published` | `true` if publish succeeded |

**Environment Variables:**
- `TOOL_REGISTRY_TOKEN` - Required for authentication (unless dry-run)

**Logic:**
1. Scan files matching `bundles` glob pattern
2. Map filenames to platform flags:
   ```
   *-darwin-arm64.*   → --darwin-arm64 <path>
   *-darwin-x86_64.*  → --darwin-x64 <path>
   *-darwin-x64.*     → --darwin-x64 <path>
   *-linux-arm64.*    → --linux-arm64 <path>
   *-linux-x86_64.*   → --linux-x64 <path>
   *-linux-x64.*      → --linux-x64 <path>
   *-win32-arm64.*    → --win32-arm64 <path>
   *-win32-x86_64.*   → --win32-x64 <path>
   *-win32-x64.*      → --win32-x64 <path>
   ```
3. Build and execute command:
   ```bash
   tool publish --multi-platform \
     --darwin-arm64 dist/my-tool-0.1.0-darwin-arm64.mcpb \
     --darwin-x64 dist/my-tool-0.1.0-darwin-x86_64.mcpb \
     --linux-arm64 dist/my-tool-0.1.0-linux-arm64.mcpb \
     --linux-x64 dist/my-tool-0.1.0-linux-x86_64.mcpb \
     --win32-arm64 dist/my-tool-0.1.0-win32-arm64.mcpb \
     --win32-x64 dist/my-tool-0.1.0-win32-x86_64.mcpb
   ```
4. Add `--dry-run` flag if `dry-run` input is true

---

### 4. `tool-action/validate` (Optional)

**Purpose:** Validate manifest.json before packing.

**Inputs:**
| Input | Default | Description |
|-------|---------|-------------|
| `strict` | `false` | Treat warnings as errors |
| `working-directory` | `.` | Directory containing manifest.json |

**Outputs:**
| Output | Description |
|--------|-------------|
| `valid` | `true` if validation passed |

**Logic:**
1. Run `tool validate` (or `tool pack --dry-run` if validate command doesn't exist)
2. Add `--strict` flag if input is true
3. Report errors/warnings

---

## Example User Workflow

This is what a vendor's release workflow would look like using these actions:

```yaml
name: Release
on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: darwin-arm64
            runner: macos-15
          - target: darwin-x86_64
            runner: macos-15-intel
          - target: linux-arm64
            runner: ubuntu-24.04-arm
          - target: linux-x86_64
            runner: ubuntu-24.04
          - target: win32-arm64
            runner: windows-11-arm
          - target: win32-x86_64
            runner: windows-2022

    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: "20.x"

      - run: npm ci --omit=dev

      - uses: zerocore-ai/tool-action/setup@v1

      - uses: zerocore-ai/tool-action/pack@v1
        id: pack
        with:
          target: ${{ matrix.target }}

      - uses: actions/upload-artifact@v4
        with:
          name: bundle-${{ matrix.target }}
          path: |
            dist/*.mcpb
            dist/*.mcpbx
            dist/*.sha256

  release:
    needs: build
    runs-on: ubuntu-24.04
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: dist
          pattern: bundle-*
          merge-multiple: true

      - uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/*.mcpb
            dist/*.mcpbx
            dist/*.sha256

  publish:
    needs: release
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4

      - uses: actions/download-artifact@v4
        with:
          path: dist
          pattern: bundle-*
          merge-multiple: true

      - uses: zerocore-ai/tool-action/setup@v1

      - uses: zerocore-ai/tool-action/publish@v1
        with:
          bundles: dist/*.mcpb
        env:
          TOOL_REGISTRY_TOKEN: ${{ secrets.TOOL_REGISTRY_TOKEN }}
```

---

## Implementation Notes

### Composite Actions

All actions should be implemented as composite actions (`runs: using: composite`) for simplicity. They only need bash scripts, no JavaScript runtime required.

### Cross-Platform Compatibility

- Use `shell: bash` on all platforms (GitHub provides bash on Windows too)
- For checksums, prefer node one-liner over `sha256sum` (not available on macOS/Windows):
  ```bash
  node -e "
    const fs = require('fs');
    const crypto = require('crypto');
    const p = process.argv[1];
    const h = crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
    fs.writeFileSync(p + '.sha256', h + '  ' + require('path').basename(p) + '\n');
  " "$BUNDLE_PATH"
  ```

### Error Handling

- Fail fast with clear error messages
- Validate inputs before executing
- Check that tool-cli is installed before pack/publish

### Testing

Create a test workflow in the repo that:
1. Creates a minimal test manifest
2. Runs setup, pack, publish (dry-run) on all platforms
3. Verifies outputs are correct

---

## Deliverables

1. **Repository:** `zerocore-ai/tool-action` (or location specified by maintainer)
2. **Actions:** `setup`, `pack`, `publish`, optionally `validate`
3. **README.md:** Documentation with examples
4. **Test workflow:** `.github/workflows/test.yml`

---

## References

- tool-cli repo: https://github.com/zerocore-ai/tool-cli
- tool-cli publish command: supports `--multi-platform` with `--darwin-arm64`, `--darwin-x64`, `--linux-arm64`, `--linux-x64`, `--win32-arm64`, `--win32-x64` flags
- MCPB spec: https://github.com/anthropics/mcpb
- Existing vendor workflows for reference:
  - `external/mongodb/.github/workflows/release.yml`
  - `external/monday/.github/workflows/release.yml`
