# Dimhouse

A build and testing framework for Ethereum consensus clients with Xatu middleware integration.

## Overview

Dimhouse allows you to:
- Clone and build Ethereum consensus clients (e.g., Lighthouse)
- Apply custom patches to integrate the Xatu observability crate
- Manage different patches for different branches/versions
- Automatically update patches when changes are made

## Directory Structure

```
├── crates/                    # Source crates to be integrated
│   └── xatu/                  # Xatu observability crate
├── diffs/                     # Patch files organized by org/repo/branch
│   └── sigp/
│       └── lighthouse/
│           └── unstable.diff  # Default patch for Lighthouse
├── dimhouse-build.sh          # Main build script
├── apply-dimhouse-patch.sh    # Helper script to apply patches
└── example-xatu-config.yaml   # Xatu configuration file
```

## Scripts

### dimhouse-build.sh

Main build script that handles the full workflow: clone, patch, build, and update diffs.

```bash
# Usage
./dimhouse-build.sh -r <org/repo> -b <branch>

# Example
./dimhouse-build.sh -r sigp/lighthouse -b unstable
./dimhouse-build.sh -r sigp/lighthouse -b fusaka-devnet-3
```

### apply-dimhouse-patch.sh

Helper script to apply patches to a repository.

```bash
# Usage
./apply-dimhouse-patch.sh <org/repo> <branch> [target_dir]

# Example
./apply-dimhouse-patch.sh sigp/lighthouse unstable
./apply-dimhouse-patch.sh sigp/lighthouse unstable /path/to/lighthouse
```

## Diff Management

Diffs are stored in `diffs/<org>/<repo>/<branch>.diff`. The system uses a fallback mechanism:
- First tries to find a branch-specific diff
- Falls back to `diffs/sigp/lighthouse/unstable.diff` if not found

When working with a new branch for the first time, the script will create a new diff file specific to that branch after a successful build.

## Configuration

The `example-xatu-config.yaml` file contains configuration for the Xatu middleware. This file is referenced by the patches applied to the consensus clients.

## Requirements

- Git
- Rust/Cargo (for building Lighthouse)
- Bash

## Notes

- The `lighthouse/` directory is git-ignored and used as a working directory
- Patches automatically exclude the xatu crate directory from diffs (since it's copied separately)
- Cargo.lock changes are excluded from diffs to avoid conflicts
