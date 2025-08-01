# Dimhouse

This project contains two main components:

- @diffs/${github_org}/${github_repo}/${branch/tag/commit}.diff - A diff file to apply to lighthouse upstread repo
- @crates/xatu - The main crate for the [xatu-sidecar](https://github.com/ethpandaops/xatu-sidecar) to be injected into the lighthouse build process

The goal of this project is inject the xatu-sidecar into the lighthouse build process, by applying a diff file to the lighthouse upstream repo.

## How is this repo used?

- The diff needs to be applied to the target lighthouse repo, if there is no diff file found, then the default is used @diffs/sigp/lighthouse/unstable.diff
- The @create/xatu crate is copied to the base of the target lighthouse repo
- `cargo build --release` should be run to build the lighthouse binary
