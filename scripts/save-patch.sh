#!/bin/bash

# save-patch.sh - Generate clean patches from manual changes to a lighthouse clone
# Usage: ./save-patch.sh [-r REPO] [-b BRANCH] [TARGET_DIR]

set -e

# Default values
ORG_REPO="sigp/lighthouse"
BRANCH="unstable"
TARGET_DIR="lighthouse"
INTERACTIVE=true
CI_MODE=false
QUIET=false

# Parse command-line arguments
ORIG_ARGS=("$@")

# Filter out --ci flag
NEW_ARGS=()
for arg in "${ORIG_ARGS[@]}"; do
    if [ "$arg" = "--ci" ]; then
        CI_MODE=true
        INTERACTIVE=false
        QUIET=true
    else
        NEW_ARGS+=("$arg")
    fi
done

set -- "${NEW_ARGS[@]}"

while getopts "r:b:nhq" opt; do
    case $opt in
        r) ORG_REPO="$OPTARG" ;;
        b) BRANCH="$OPTARG" ;;
        n) INTERACTIVE=false ;;
        q) QUIET=true ;;
        h)
            echo "Usage: $0 [-r REPO] [-b BRANCH] [-n] [-q] [--ci] [TARGET_DIR]"
            echo "  -r REPO    GitHub org/repo (default: sigp/lighthouse)"
            echo "  -b BRANCH  Branch/tag/commit (default: unstable)"
            echo "  -n         Non-interactive mode"
            echo "  -q         Quiet mode"
            echo "  --ci       CI mode (non-interactive, quiet)"
            echo "  TARGET_DIR Directory to save patch from (default: lighthouse)"
            exit 0
            ;;
        \?) echo "Invalid option: -$OPTARG" >&2; exit 1 ;;
    esac
done

shift $((OPTIND-1))

if [ $# -gt 0 ]; then
    TARGET_DIR="$1"
fi

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Validate target directory
if [ ! -d "$TARGET_DIR" ]; then
    echo "Error: Target directory '$TARGET_DIR' does not exist"
    exit 1
fi

cd "$TARGET_DIR"

if [ ! -d ".git" ]; then
    echo "Error: Target directory is not a git repository"
    exit 1
fi

# Extract org and repo
ORG=$(echo "$ORG_REPO" | cut -d'/' -f1)
REPO=$(echo "$ORG_REPO" | cut -d'/' -f2)

PATCH_DIR="$REPO_ROOT/patches/$ORG/$REPO"
PATCH_FILE="$PATCH_DIR/$BRANCH.patch"

if [ "$QUIET" = false ]; then
    echo "========================================="
    echo "  Save Patch Script"
    echo "========================================="
    echo "Repository: $ORG_REPO"
    echo "Branch: $BRANCH"
    echo "Target directory: $(pwd)"
    echo "Patch file: $PATCH_FILE"
    echo ""
fi

# Step 1: Remove overlay files (these are copied, not patched)
[ "$QUIET" = false ] && echo "Removing overlay files from diff..." || true
rm -rf xatu

# Step 2: Restore files managed by scripts (not in patch)
[ "$QUIET" = false ] && echo "Restoring script-managed files to upstream state..." || true
git checkout HEAD -- Dockerfile 2>/dev/null || true
git checkout HEAD -- beacon_node/Cargo.toml 2>/dev/null || true
git checkout HEAD -- beacon_node/network/Cargo.toml 2>/dev/null || true
git checkout HEAD -- lighthouse/Cargo.toml 2>/dev/null || true
git checkout HEAD -- .gitignore 2>/dev/null || true

# Step 3: Restore Cargo.lock to upstream state
if git status --porcelain | grep -q "Cargo.lock"; then
    [ "$QUIET" = false ] && echo "Restoring Cargo.lock to upstream state..." || true
    git checkout HEAD -- Cargo.lock 2>/dev/null || true
fi

# Step 4: Rename .yml.disabled back to .yml
[ "$QUIET" = false ] && echo "Restoring disabled workflows..." || true
for f in .github/workflows/*.yml.disabled; do
    [ -f "$f" ] || continue
    mv "$f" "${f%.disabled}"
done

# Step 5: Remove CI overlay files
rm -f Dockerfile.ethpandaops

# Step 6: Remove any .rej or .orig files
find . -name "*.rej" -o -name "*.orig" | xargs rm -f 2>/dev/null || true

# Step 7: Check if there are any changes to save
if [ -z "$(git status --porcelain)" ]; then
    if [ "$CI_MODE" = true ]; then
        [ "$QUIET" = false ] && echo "No changes to save" || true
        exit 2
    else
        echo ""
        echo "No changes detected in the repository"
        echo "  Make your manual changes first, then run this script again"
        exit 1
    fi
fi

# Step 8: Show what will be included in the patch
if [ "$QUIET" = false ]; then
    echo ""
    echo "Changes to be saved in patch:"
    echo "--------------------------------"
    git status --short
    echo "--------------------------------"
    echo ""
fi

# Step 9: Create patch directory if it doesn't exist
mkdir -p "$PATCH_DIR"

# Step 10: Generate the patch
[ "$QUIET" = false ] && echo "Generating patch..." || true
git diff --no-color --no-ext-diff > "$PATCH_FILE"

# Step 11: Check if patch was created successfully
if [ ! -s "$PATCH_FILE" ]; then
    echo "Error: Failed to create patch or patch is empty"
    exit 1
fi

# Step 12: Show patch statistics
PATCH_LINES=$(wc -l < "$PATCH_FILE")
PATCH_SIZE=$(du -h "$PATCH_FILE" | cut -f1)
ADDED_LINES=$(grep -c "^+" "$PATCH_FILE" 2>/dev/null || echo 0)
REMOVED_LINES=$(grep -c "^-" "$PATCH_FILE" 2>/dev/null || echo 0)

if [ "$QUIET" = false ]; then
    echo ""
    echo "Patch saved successfully!"
    echo ""
    echo "Patch statistics:"
    echo "  File: $PATCH_FILE"
    echo "  Size: $PATCH_SIZE"
    echo "  Total lines: $PATCH_LINES"
    echo "  Added lines: $ADDED_LINES"
    echo "  Removed lines: $REMOVED_LINES"
    echo ""

    echo "Next steps:"
    echo "  1. Review the patch: less \"$PATCH_FILE\""
    echo "  2. Test applying it: ./scripts/apply-dimhouse-patch.sh $ORG_REPO $BRANCH $TARGET_DIR"
    echo "  3. Build with it: ./scripts/dimhouse-build.sh -r $ORG_REPO -b $BRANCH"
    echo ""
else
    echo "$PATCH_FILE"
fi

if [ "$INTERACTIVE" = true ]; then
    read -p "Would you like to preview the patch? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo ""
        echo "Patch preview (first 50 lines):"
        echo "================================"
        head -50 "$PATCH_FILE"
        if [ "$PATCH_LINES" -gt 50 ]; then
            echo ""
            echo "... (showing first 50 of $PATCH_LINES lines)"
        fi
    fi
fi

[ "$QUIET" = false ] && echo "" && echo "Done!" || true
