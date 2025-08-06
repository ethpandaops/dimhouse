# Dimhouse

Automated patch management and build system for integrating Xatu Sidecar observability into Lighthouse.

## Overview

Dimhouse is a patch management system that integrates [Xatu Sidecar](https://github.com/ethpandaops/xatu-sidecar) observability into the [Lighthouse](https://github.com/sigp/lighthouse) Ethereum consensus client. It maintains patches that inject the Xatu Sidecar crate into Lighthouse builds, enabling enhanced network monitoring and metrics collection for the [Xatu](https://github.com/ethpandaops/xatu) data collection pipeline.

### Key Features

- **Xatu Sidecar Integration**: Seamlessly adds Xatu Sidecar observability capabilities to Lighthouse
- **Patch Management**: Maintains patches for different Lighthouse versions (branches, tags, commits)
- **Automated Updates**: CI/CD workflows to keep patches current with upstream changes
- **Multi-version Support**: Track and build multiple versions simultaneously
- **Release Automation**: Automatic GitHub releases for updated patches

## Directory Structure

```
├── .github/workflows/         # GitHub Actions workflows
│   ├── check-patches.yml      # Automated patch checking and updating
│   ├── add-patch.yml          # Manual patch addition workflow
│   └── list-patches.yml       # List all available patches
├── crates/                    # Source crates to be integrated
│   └── xatu/                  # Xatu Sidecar crate for Lighthouse integration
├── patches/                   # Patch files organized by org/repo/ref
│   └── sigp/
│       └── lighthouse/
│           └── unstable.patch # Default patch for Lighthouse
├── dimhouse-build.sh          # Main build script
├── apply-dimhouse-patch.sh    # Helper script to apply patches
└── example-xatu-config.yaml   # Xatu configuration file
```

## Scripts

### dimhouse-build.sh

Main build script that handles the full workflow: clone, patch, build, and update patches.

```bash
# Usage
./dimhouse-build.sh -r <org/repo> -b <branch/tag/commit> [--ci]

# Examples
./dimhouse-build.sh -r sigp/lighthouse -b unstable
./dimhouse-build.sh -r sigp/lighthouse -b v4.5.0
./dimhouse-build.sh -r sigp/lighthouse -b a1b2c3d

# CI mode (non-interactive, auto-approves changes)
./dimhouse-build.sh -r sigp/lighthouse -b unstable --ci
```

**Options:**
- `-r, --repo`: Repository in format `org/repo`
- `-b, --branch`: Branch name, tag, or commit hash
- `--ci`: Run in CI mode (non-interactive, auto-clean, auto-update patches)

### apply-dimhouse-patch.sh

Helper script to apply patches to an existing repository.

```bash
# Usage
./apply-dimhouse-patch.sh <org/repo> <branch/tag/commit> [target_dir]

# Examples
./apply-dimhouse-patch.sh sigp/lighthouse unstable
./apply-dimhouse-patch.sh sigp/lighthouse v4.5.0 /path/to/lighthouse
```

## Patch Management

### Patch Storage

Patches are stored in `patches/<org>/<repo>/<ref>.patch` where `ref` can be:
- Branch name: `unstable.patch`
- Tag: `v4.5.0.patch`
- Commit: `a1b2c3d.patch`

### Fallback Mechanism

The system uses a fallback mechanism when applying patches:
1. First tries to find an exact match: `patches/<org>/<repo>/<ref>.patch`
2. Falls back to default: `patches/sigp/lighthouse/unstable.patch`

### Automatic Updates

When running `dimhouse-build.sh`:
- If the build succeeds and the patch has changed, you'll be prompted to update it
- In CI mode (`--ci`), patches are automatically updated without prompting
- New patches are created automatically for new branches/tags/commits

## GitHub Actions Workflows

### Automated Patch Checking (`check-patches.yml`)

Runs daily (2 AM UTC) or manually to:
- Discover all existing patches
- Build each patch in parallel
- Auto-commit updated patches if changes are detected
- Create GitHub releases for each updated patch

**Triggers:**
- Schedule: Daily at 2 AM UTC
- Manual: Via GitHub Actions UI

### Manual Patch Addition (`add-patch.yml`)

Allows manual addition of new patches via GitHub Actions UI.

**Inputs:**
- `repository`: Repository in `org/repo` format
- `ref`: Branch, tag, or commit hash
- `patch_name`: Optional custom patch filename
- `force_rebuild`: Force overwrite existing patches

**Example:** Adding a patch for a specific Lighthouse release:
1. Go to Actions → "Add New Patch"
2. Enter: `sigp/lighthouse` and `v4.5.0`
3. Run workflow

### Patch Inventory (`list-patches.yml`)

Lists all available patches in a table format.

**Triggers:**
- Manual: Via GitHub Actions UI
- Automatic: When patches are modified

## Releases

The CI system automatically creates GitHub releases for updated patches with tags following the format:
```
<org>-<repo>-<ref>-<short_commit_hash>
```

Example: `sigp-lighthouse-unstable-a3f5e92`

Each release includes:
- The patch file as an artifact
- Build details and commit information
- Application instructions

## Configuration

The `example-xatu-config.yaml` file contains configuration for the Xatu Sidecar. This file is referenced by the environment variable `XATU_CONFIG` when running the patched Lighthouse binary with Xatu Sidecar integration.

## Requirements

- Git
- Rust/Cargo (for building Lighthouse)
- Bash
- GitHub CLI (`gh`) for release creation in CI

## Notes

- The `lighthouse/` directory is git-ignored and used as a working directory
- Patches automatically exclude the xatu crate directory (it's copied separately)
- Cargo.lock changes are excluded from patches to avoid conflicts
- CI mode (`--ci`) enables fully automated operation without user prompts

## Usage Examples

### Local Development

```bash
# Build Lighthouse unstable with patches
./dimhouse-build.sh -r sigp/lighthouse -b unstable

# Apply patch to existing clone
cd /path/to/lighthouse
../dimhouse/apply-dimhouse-patch.sh sigp/lighthouse unstable .
```

### Adding Support for New Versions

```bash
# Build and create patch for a specific tag
./dimhouse-build.sh -r sigp/lighthouse -b v4.5.0

# Build and create patch for a specific commit
./dimhouse-build.sh -r sigp/lighthouse -b a1b2c3d
```

### CI/CD Integration

The GitHub Actions workflows handle:
- Daily patch updates for all tracked versions
- Manual patch addition through the UI
- Automatic release creation with proper versioning
- Patch inventory maintenance

## Contributing

1. Fork the repository
2. Add or modify patches as needed
3. Test locally with `dimhouse-build.sh`
4. Submit a pull request

Patches should be tested to ensure they:
- Apply cleanly to the target version
- Build successfully
- Maintain the intended Xatu Sidecar integration functionality