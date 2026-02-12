# Dimhouse

This project contains three main components:

- `patches/${github_org}/${github_repo}/${branch/tag/commit}.patch` - Patch files (Rust source only) to apply to lighthouse upstream repo
- `overlay/xatu` - The xatu crate for the [xatu-sidecar](https://github.com/ethpandaops/xatu-sidecar) to be injected into the lighthouse build process
- `ci/` - Dockerfile and CI helpers (copied over upstream files during apply)

The goal of this project is to inject the xatu-sidecar into the lighthouse build process, by applying a patch file to the lighthouse upstream repo.

## Project structure

```
patches/         - Slim patches (only Rust source diffs)
overlay/xatu/    - Xatu crate overlay (copied into lighthouse during apply)
ci/              - Dockerfile.ethpandaops and workflow helpers
scripts/         - Build, apply, save, update-deps, and validate scripts
```

## How is this repo used?

1. `scripts/apply-dimhouse-patch.sh` applies patches, copies overlay/xatu, copies ci/Dockerfile.ethpandaops, injects Cargo.toml deps via `scripts/update-deps.sh`, and disables upstream workflows
2. `scripts/dimhouse-build.sh` orchestrates clone + apply + build (supports `--skip-build` for Docker CI)
3. `scripts/save-patch.sh` strips overlay/CI/dep artifacts and generates a clean patch

## Testing with Kurtosis

`kurtosis-config.yaml` defines a local testnet with a dimhouse (lighthouse+xatu) node and a vanilla lighthouse node.
The xatu config is inlined via `extra_files` with stdout output so events appear in container logs.

```bash
# Quick test (assumes ethpandaops/dimhouse:latest image exists):
kurtosis run github.com/ethpandaops/ethereum-package --args-file kurtosis-config.yaml --enclave dimhouse

# View xatu events:
kurtosis service logs dimhouse cl-1-lighthouse-nethermind

# Cleanup:
kurtosis enclave rm -f dimhouse
```

Key notes:
- The xatu config `type: stdout` requires a dummy `config.address` field to satisfy the Rust config parser (the Go side ignores it for stdout)
- The `vc_image` must match `cl_image` since lighthouse VC shares the same binary
- No ENTRYPOINT in the Docker image -- kurtosis passes `lighthouse beacon_node ...` / `lighthouse vc ...` as the full command
- `libxatu.so` must be in the image at a path covered by `LD_LIBRARY_PATH` (e.g. `/usr/local/lib`)
- Update `overlay/xatu/src/libxatu.so` when bumping xatu-sidecar version (build.rs only downloads if missing)
