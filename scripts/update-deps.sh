#!/bin/bash
# update-deps.sh - Inject dimhouse-specific Cargo.toml dependencies and features
# Run this from the upstream lighthouse clone directory after applying patches
set -e

echo "Injecting dimhouse-specific Cargo.toml changes..."

# 1. beacon_node/network/Cargo.toml: Add xatu dependency before [dev-dependencies]
if ! grep -q 'xatu = { path' beacon_node/network/Cargo.toml; then
    echo "  Adding xatu dependency to beacon_node/network/Cargo.toml"
    sed -i '/^\[dev-dependencies\]/i \# Xatu dependency\nxatu = { path = "../../xatu" }\n' beacon_node/network/Cargo.toml
fi

# 2. beacon_node/Cargo.toml: Add disable-backfill feature
if ! grep -q 'disable-backfill' beacon_node/Cargo.toml; then
    echo "  Adding disable-backfill feature to beacon_node/Cargo.toml"
    sed -i '/^testing = \[\]/a disable-backfill = ["network/disable-backfill"]' beacon_node/Cargo.toml
fi

# 3. beacon_node/Cargo.toml: Add network workspace dependency
if ! grep -q '^network = { workspace = true }' beacon_node/Cargo.toml; then
    echo "  Adding network dependency to beacon_node/Cargo.toml"
    sed -i '/^\[dependencies\]/a network = { workspace = true }' beacon_node/Cargo.toml
fi

# 4. lighthouse/Cargo.toml: Add disable-backfill feature
if ! grep -q 'disable-backfill' lighthouse/Cargo.toml; then
    echo "  Adding disable-backfill feature to lighthouse/Cargo.toml"
    sed -i '/^beacon-node-redb = \["store\/redb"\]/a # Disable historical block backfilling during sync.\ndisable-backfill = ["beacon_node/disable-backfill"]' lighthouse/Cargo.toml
fi

echo "Cargo.toml changes injected successfully"
