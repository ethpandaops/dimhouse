# Dimhouse

This project contains two main components:

- @patches/${github_org}/${github_repo}/${branch/tag/commit}.patch - A patch file to apply to lighthouse upstream repo
- @crates/xatu - The main crate for the [xatu-sidecar](https://github.com/ethpandaops/xatu-sidecar) to be injected into the lighthouse build process

The goal of this project is to inject the xatu-sidecar into the lighthouse build process, by applying a patch file to the lighthouse upstream repo.

## How is this repo used?

- The patch needs to be applied to the target lighthouse repo, if there is no patch file found, then the default is used @patches/sigp/lighthouse/unstable.patch
- The @crates/xatu crate is copied to the base of the target lighthouse repo
- `cargo build --release` should be run to build the lighthouse binary
