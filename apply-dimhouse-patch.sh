#!/bin/bash

set -e

# This script applies the dimhouse patch (xatu crate + diff) to a repository
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

# Get the script's directory (where diffs/ and crates/ are located)
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

# Find the appropriate diff file with fallback logic
find_diff_file() {
    local org="$1"
    local repo="$2"
    local branch="$3"
    
    # Try exact match first
    local exact_diff="$SCRIPT_DIR/diffs/$org/$repo/$branch.diff"
    if [ -f "$exact_diff" ]; then
        echo "$exact_diff"
        return 0
    fi
    
    # Fallback to default
    local default_diff="$SCRIPT_DIR/diffs/sigp/lighthouse/unstable.diff"
    if [ -f "$default_diff" ]; then
        echo "Diff file not found at diffs/$org/$repo/$branch.diff, using default..." >&2
        echo "$default_diff"
        return 0
    fi
    
    return 1
}

# Find the diff file
DIFF_FILE=$(find_diff_file "$ORG" "$REPO" "$BRANCH")
if [ -z "$DIFF_FILE" ]; then
    echo "Error: No diff file found"
    exit 1
fi

echo "Using diff file: $DIFF_FILE"

# Apply the diff
echo "Applying diff..."
if ! git apply "$DIFF_FILE"; then
    echo "Error: Failed to apply diff"
    echo "This might happen if:"
    echo "  - The diff is already applied"
    echo "  - The repository is not clean"
    echo "  - The diff is incompatible with the current branch"
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
echo "  - Applied diff: $DIFF_FILE"
echo "  - Copied xatu crate to: $(pwd)/xatu"