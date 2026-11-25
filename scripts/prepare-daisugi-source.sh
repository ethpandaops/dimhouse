#!/bin/bash
# Prepares ./lighthouse for a daisugi-devnet-0 dimhouse build.
#
# daisugi-devnet-0 runs Lighthouse v8.2.2 (e423a667) plus the EIP-8288 patch
# series from asanso/AA-PQ (aggregation/frames/consensus/eip8288-lean). That
# package pins the source archive and every patch by sha256 and checks the
# resulting tree, so this rebuilds the exact source of
# ethpandaops/lighthouse:daisugi-devnet-0, then applies the dimhouse v8.2.2
# patch and overlay on top.
set -euo pipefail

AA_PQ_COMMIT=4b0a11e0e7d261bf47a2e78589bcdff050703412
PACKAGE=aggregation/frames/consensus/eip8288-lean

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

git clone --quiet --filter=blob:none --no-checkout https://github.com/asanso/AA-PQ "$WORK/aa-pq"
git -C "$WORK/aa-pq" sparse-checkout set "$PACKAGE"
git -C "$WORK/aa-pq" checkout --quiet "$AA_PQ_COMMIT"

cd "$WORK/aa-pq/$PACKAGE"
revision=$(jq -r .lighthouse.revision sources.lock.json)
curl -fsSL -o "$WORK/lighthouse.tar.gz" "https://github.com/sigp/lighthouse/archive/$revision.tar.gz"
rm -rf "$ROOT/lighthouse"
python3 apply.py lighthouse "$WORK/lighthouse.tar.gz" "$ROOT/lighthouse"

# apply-dimhouse-patch.sh needs a git work tree.
cd "$ROOT/lighthouse"
git init --quiet
git add -A
git -c user.name=dimhouse -c user.email=dimhouse@ethpandaops.io commit --quiet -m "daisugi-devnet-0 lighthouse source"

cd "$ROOT"
./scripts/apply-dimhouse-patch.sh sigp/lighthouse v8.2.2 lighthouse
