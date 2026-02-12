# Dimhouse

Patch-based overlay for integrating [Xatu Sidecar](https://github.com/ethpandaops/xatu-sidecar) observability into [Lighthouse](https://github.com/sigp/lighthouse).

## Overview

Dimhouse uses a **patch + overlay** approach instead of maintaining a full fork. The repo stores only custom code and small patches; upstream Lighthouse is cloned fresh each build.

### Repository Structure

```
dimhouse/
├── overlay/                      # Custom code (copied into upstream clone)
│   └── xatu/                     # Xatu Sidecar crate for Lighthouse integration
├── patches/
│   └── sigp/lighthouse/
│       ├── unstable.patch              # Base patch: gossip hooks, xatu init, CLI flag
│       └── unstable-01-optimistic.patch # Extension: bypass EL validation for observation
├── ci/
│   ├── Dockerfile.ethpandaops    # Custom Dockerfile (replaces upstream)
│   └── disable-upstream-workflows.sh
├── .github/workflows/
│   ├── check-patches.yml         # Daily: verify patches apply + build
│   ├── docker.yml                # On push/release: build + push Docker image
│   └── validate-patches.yml      # On PR: validate patch file structure
├── scripts/
│   ├── dimhouse-build.sh         # Full orchestrator: clone -> patch -> build
│   ├── apply-dimhouse-patch.sh   # Apply patches + overlay + deps
│   ├── save-patch.sh             # Regenerate patches from modified clone
│   ├── update-deps.sh            # Cargo.toml dep/feature injection via sed
│   └── validate-patch.sh         # Patch file structural validation
├── example-xatu-config.yaml      # Xatu configuration template
└── .gitignore                    # Ignore lighthouse/ working directory
```

## Quick Start

### Build

```bash
# Full build: clone upstream -> apply patches -> build binary
./scripts/dimhouse-build.sh -r sigp/lighthouse -b unstable

# The binary will be at lighthouse/target/release/lighthouse
```

### Docker

```bash
# Prepare patched source (skip Rust build, let Docker handle it)
./scripts/dimhouse-build.sh -r sigp/lighthouse -b unstable --skip-build

# Build Docker image from the patched source
cd lighthouse && docker build -t ethpandaops/dimhouse:latest .
```

### Run

```bash
lighthouse beacon_node --xatu-config /path/to/xatu-config.yaml [other options]
```

The configuration file should be based on [`example-xatu-config.yaml`](example-xatu-config.yaml).

## Scripts

| Script | Purpose |
|---|---|
| `dimhouse-build.sh` | Full orchestrator: clone upstream, apply patches + overlay, build |
| `apply-dimhouse-patch.sh` | Apply patches to an existing lighthouse clone + copy overlay + deps |
| `save-patch.sh` | Regenerate patches from a modified lighthouse clone |
| `update-deps.sh` | Inject Cargo.toml dependencies and features via sed |
| `validate-patch.sh` | Validate patch file structure (hunk counts, etc.) |
| `disable-upstream-workflows.sh` | Rename upstream CI workflows to `.disabled` |

### dimhouse-build.sh

```bash
./scripts/dimhouse-build.sh -r <org/repo> -b <branch> [-c <commit>] [--ci] [--skip-build]
```

**Options:**
- `-r, --repo`: Repository in format `org/repo`
- `-b, --branch`: Branch name, tag, or commit hash
- `-c, --commit`: Pin to specific upstream commit SHA
- `--ci`: CI mode (non-interactive, auto-clean, auto-update patches)
- `--skip-build`: Skip `cargo build`, exit after applying patches (for Docker CI)

## How It Works

### Custom Code as Overlay

`overlay/xatu/` is the Xatu Sidecar crate, **copied** into the upstream clone at build time. It is never part of the patch.

### Dependencies via Script

Instead of patching `Cargo.toml` files (which break on every upstream dependency change), `update-deps.sh` uses idempotent sed commands to inject:

- `xatu = { path = "../../xatu" }` into `beacon_node/network/Cargo.toml`
- `network = { workspace = true }` and `disable-backfill` feature into `beacon_node/Cargo.toml`
- `disable-backfill` feature into `lighthouse/Cargo.toml`

### CI Workflow Disabling

Instead of patching workflow renames, a simple script renames all non-dimhouse workflows to `.disabled`.

### Dockerfile as Overlay

Instead of patching the upstream Dockerfile, `ci/Dockerfile.ethpandaops` is copied over the upstream Dockerfile during apply.

### Patches

The actual patch surface is minimal (Rust source only):
- **`unstable.patch`** (~490 lines): Adds gossip message size tracking, xatu chain initialization, gossip event forwarding, `--xatu-config` CLI flag, and rpath setup
- **`unstable-01-optimistic.patch`** (~130 lines): Bypasses EL validation for observation-only nodes

## Development

### Adding a new feature

#### New files (overlay)

Self-contained new code goes in `overlay/xatu/`. These files are copied verbatim into the upstream clone at build time.

```bash
vim overlay/xatu/new_feature.rs
git add overlay/
git commit -m "feat: add new feature"
```

#### Modifying upstream files (patch)

If your feature requires changing existing upstream Rust source:

```bash
# 1. Build to get the working upstream clone
./scripts/dimhouse-build.sh -r sigp/lighthouse -b unstable

# 2. Edit upstream files in the clone
vim lighthouse/beacon_node/network/src/router.rs

# 3. Regenerate the patch
./scripts/save-patch.sh -r sigp/lighthouse -b unstable lighthouse

# 4. Commit the updated patch
git add patches/
git commit -m "feat: add new-feature wiring to base patch"
```

Changes are folded into `unstable.patch`. If the change is logically separate, create a new extension patch named `unstable-02-your-feature.patch` and it will be picked up automatically in alphabetical order.

#### New dependency

Edit `scripts/update-deps.sh` and add the sed injection for the new Cargo.toml entry.

> **Tip:** Most features are overlay files + maybe a new dependency. Touching upstream files should be rare and minimal -- the less patch surface, the fewer sync conflicts.

### Fixing a patch conflict

When upstream changes the same lines our patches touch, `apply-dimhouse-patch.sh` will fail. To fix:

```bash
# 1. Run the build -- it will show exactly which hunks failed
./scripts/dimhouse-build.sh -r sigp/lighthouse -b unstable

# 2. Fix the conflicts in the upstream clone
vim lighthouse/beacon_node/network/src/router.rs

# 3. Regenerate the patch
./scripts/save-patch.sh -r sigp/lighthouse -b unstable lighthouse

# 4. Commit the updated patch
git add patches/
git commit -m "fix: update patches for upstream changes"
```

### Dropping a patch

Extension patches are independently droppable. If upstream incorporates a fix:

```bash
git rm patches/sigp/lighthouse/unstable-01-optimistic.patch
git commit -m "chore: drop optimistic patch, merged upstream"
```

## CI

| Workflow | Trigger | What it does |
|---|---|---|
| `check-patches.yml` | Daily (cron) | Clones upstream, applies patches, builds. Auto-commits if patches needed updating |
| `docker.yml` | Push to master / release | Builds + pushes multi-arch Docker image to `ethpandaops/dimhouse:<tag>` |
| `validate-patches.yml` | PR | Validates patch file structure (hunk counts, etc.) |

## Requirements

- Rust/Cargo 1.88+ (for building Lighthouse with edition2024 support)
- Git
- Bash
- cmake (required for building native dependencies)
- GitHub CLI (`gh`) for release creation in CI

### macOS-Specific Requirements

Building on macOS requires additional setup since pre-built xatu-sidecar libraries are only available for Linux:

1. **Go 1.21+** - Required to build xatu-sidecar from source
2. **Xcode Command Line Tools** - For `install_name_tool`

## Local macOS Development

The xatu-sidecar releases only include Linux binaries. For local macOS development, you need to build the library from source:

### 1. Build xatu-sidecar library

```bash
cd /tmp
git clone https://github.com/ethpandaops/xatu-sidecar.git
cd xatu-sidecar
CGO_ENABLED=1 go build -buildmode=c-shared -o libxatu.dylib .

# Fix the install name for proper dynamic loading
install_name_tool -id "@rpath/libxatu.dylib" libxatu.dylib
```

### 2. Build lighthouse with dimhouse patches

```bash
./scripts/dimhouse-build.sh -r sigp/lighthouse -b unstable

# The build will fail trying to download darwin binary, so manually copy the library:
cp /tmp/xatu-sidecar/libxatu.dylib lighthouse/xatu/src/

# Build lighthouse
cd lighthouse
cargo build --release
```

### 3. Run the binary

The built binary will be at `lighthouse/target/release/lighthouse`. The `libxatu.dylib` must be in the same directory as the binary:

```bash
cp lighthouse/xatu/src/libxatu.dylib lighthouse/target/release/
./lighthouse/target/release/lighthouse --version
```

### Troubleshooting macOS Builds

**Library not loaded error**: Ensure `libxatu.dylib` is in the same directory as the lighthouse binary and has the correct install name (`@rpath/libxatu.dylib`). Check with:
```bash
otool -D libxatu.dylib  # Should show: @rpath/libxatu.dylib
otool -L lighthouse | grep xatu  # Should show: @rpath/libxatu.dylib
```

**Rust version too old**: Update with `rustup update stable` (needs 1.88+)

**cmake not found**: Install with `brew install cmake`
