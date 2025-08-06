#!/bin/bash

set -e

# This script applies the dimhouse patch (xatu crate + patch file) to a repository
# Usage: ./apply-dimhouse-patch.sh <org/repo> <branch> [target_dir]

if [ $# -lt 2 ]; then
    echo "Usage: $0 <org/repo> <branch> [target_dir]"
    echo "Example: $0 sigp/lighthouse unstable"
    echo "         $0 sigp/lighthouse unstable /path/to/lighthouse"
    exit 1
fi

# Parse org/repo
IFS='/' read -ra REPO_PARTS <<< "$1"
if [ ${#REPO_PARTS[@]} -ne 2 ]; then
    echo "Error: Repository must be in format 'org/repo'"
    exit 1
fi
ORG="${REPO_PARTS[0]}"
REPO="${REPO_PARTS[1]}"
BRANCH="$2"
TARGET_DIR="${3:-lighthouse}"  # Default to "lighthouse" if not specified

# Get the script's directory (where patches/ and crates/ are located)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Change to target directory
if [ ! -d "$TARGET_DIR" ]; then
    echo "Error: Target directory '$TARGET_DIR' does not exist"
    exit 1
fi

cd "$TARGET_DIR"

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    echo "Error: Target directory is not a git repository"
    exit 1
fi

# Find the appropriate patch file with fallback logic
find_patch_file() {
    local org="$1"
    local repo="$2"
    local branch="$3"
    
    # Try exact match first
    local exact_patch="$SCRIPT_DIR/patches/$org/$repo/$branch.patch"
    if [ -f "$exact_patch" ]; then
        echo "$exact_patch"
        return 0
    fi
    
    # Fallback to default
    local default_patch="$SCRIPT_DIR/patches/sigp/lighthouse/unstable.patch"
    if [ -f "$default_patch" ]; then
        echo "Patch file not found at patches/$org/$repo/$branch.patch, using default..." >&2
        echo "$default_patch"
        return 0
    fi
    
    return 1
}

# Find the patch file
PATCH_FILE=$(find_patch_file "$ORG" "$REPO" "$BRANCH")
if [ -z "$PATCH_FILE" ]; then
    echo "Error: No patch file found"
    exit 1
fi

echo "Using patch file: $PATCH_FILE"

# Apply the patch
echo "Applying patch..."
if ! git apply "$PATCH_FILE"; then
    echo "Error: Failed to apply patch"
    echo "This might happen if:"
    echo "  - The patch is already applied"
    echo "  - The repository is not clean"
    echo "  - The patch is incompatible with the current branch"
    exit 1
fi

# Copy the xatu crate
echo "Copying xatu crate..."
if [ ! -d "$SCRIPT_DIR/crates/xatu" ]; then
    echo "Error: xatu crate not found at $SCRIPT_DIR/crates/xatu"
    exit 1
fi

cp -r "$SCRIPT_DIR/crates/xatu" .

echo "Successfully applied dimhouse patch!"
echo "  - Applied patch: $PATCH_FILE"
echo "  - Copied xatu crate to: $(pwd)/xatu"