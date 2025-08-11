#!/bin/bash

# save-patch.sh - Generate a clean patch from manual changes to lighthouse
# Usage: ./save-patch.sh [-r REPO] [-b BRANCH] [TARGET_DIR]
#   -r REPO: GitHub org/repo (default: sigp/lighthouse)
#   -b BRANCH: Branch/tag/commit (default: unstable)
#   TARGET_DIR: Directory to save patch from (default: lighthouse)

set -e

# Default values
ORG_REPO="sigp/lighthouse"
BRANCH="unstable"
TARGET_DIR="lighthouse"
INTERACTIVE=true
CI_MODE=false
QUIET=false

# Parse command-line arguments
# Save original arguments
ORIG_ARGS=("$@")

# First, filter out --ci flag and set it
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

# Reset arguments without --ci for getopts
set -- "${NEW_ARGS[@]}"

# Now parse short options
while getopts "r:b:nhq" opt; do
    case $opt in
        r)
            ORG_REPO="$OPTARG"
            ;;
        b)
            BRANCH="$OPTARG"
            ;;
        n)
            INTERACTIVE=false
            ;;
        q)
            QUIET=true
            ;;
        h)
            echo "Usage: $0 [-r REPO] [-b BRANCH] [-n] [-q] [--ci] [TARGET_DIR]"
            echo "  -r REPO    GitHub org/repo (default: sigp/lighthouse)"
            echo "  -b BRANCH  Branch/tag/commit (default: unstable)"
            echo "  -n         Non-interactive mode (skip preview prompt)"
            echo "  -q         Quiet mode (minimal output)"
            echo "  --ci       CI mode (non-interactive, quiet, return 2 if no changes)"
            echo "  TARGET_DIR Directory to save patch from (default: lighthouse)"
            exit 0
            ;;
        \?)
            echo "Invalid option: -$OPTARG" >&2
            exit 1
            ;;
    esac
done

# Shift to get positional arguments
shift $((OPTIND-1))

# Get target directory from positional argument if provided
if [ $# -gt 0 ]; then
    TARGET_DIR="$1"
fi

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Validate target directory
if [ ! -d "$TARGET_DIR" ]; then
    echo "Error: Target directory '$TARGET_DIR' does not exist"
    exit 1
fi

# Change to target directory
cd "$TARGET_DIR"

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    echo "Error: Target directory is not a git repository"
    exit 1
fi

# Extract org and repo from the combined string
ORG=$(echo "$ORG_REPO" | cut -d'/' -f1)
REPO=$(echo "$ORG_REPO" | cut -d'/' -f2)

# Define patch file path
PATCH_DIR="$SCRIPT_DIR/patches/$ORG/$REPO"
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

# Step 1: Clean up - Remove xatu crate if it exists
if [ -d "xatu" ]; then
    [ "$QUIET" = false ] && echo "→ Removing xatu crate directory..."
    rm -rf xatu
fi

# Step 2: Clean up - Remove Cargo.lock from git tracking if modified
if git status --porcelain | grep -q "Cargo.lock"; then
    [ "$QUIET" = false ] && echo "→ Restoring Cargo.lock to original state..."
    git checkout HEAD -- Cargo.lock 2>/dev/null || true
fi

# Step 3: Remove any .rej or .orig files
[ "$QUIET" = false ] && echo "→ Cleaning up .rej and .orig files..."
find . -name "*.rej" -o -name "*.orig" | xargs rm -f 2>/dev/null || true

# Step 4: Check if there are any changes to save
if [ -z "$(git status --porcelain)" ]; then
    if [ "$CI_MODE" = true ]; then
        # In CI mode, exit with code 2 to indicate no changes (not an error, but different from success)
        [ "$QUIET" = false ] && echo "No changes to save"
        exit 2
    else
        echo ""
        echo "⚠ No changes detected in the repository"
        echo "  Make your manual changes first, then run this script again"
        exit 1
    fi
fi

# Step 5: Show what will be included in the patch
if [ "$QUIET" = false ]; then
    echo ""
    echo "→ Changes to be saved in patch:"
    echo "--------------------------------"
    git status --short
    echo "--------------------------------"
    echo ""
fi

# Step 6: Create patch directory if it doesn't exist
mkdir -p "$PATCH_DIR"

# Step 7: Generate the patch
[ "$QUIET" = false ] && echo "→ Generating patch..."
git diff --no-color --no-ext-diff > "$PATCH_FILE"

# Step 8: Check if patch was created successfully
if [ ! -s "$PATCH_FILE" ]; then
    echo "Error: Failed to create patch or patch is empty"
    exit 1
fi

# Step 9: Show patch statistics
PATCH_LINES=$(wc -l < "$PATCH_FILE")
PATCH_SIZE=$(du -h "$PATCH_FILE" | cut -f1)
ADDED_LINES=$(grep -c "^+" "$PATCH_FILE" 2>/dev/null || echo 0)
REMOVED_LINES=$(grep -c "^-" "$PATCH_FILE" 2>/dev/null || echo 0)

if [ "$QUIET" = false ]; then
    echo ""
    echo "✓ Patch saved successfully!"
    echo ""
    echo "Patch statistics:"
    echo "  • File: $PATCH_FILE"
    echo "  • Size: $PATCH_SIZE"
    echo "  • Total lines: $PATCH_LINES"
    echo "  • Added lines: $ADDED_LINES"
    echo "  • Removed lines: $REMOVED_LINES"
    echo ""
    
    # Step 10: Provide next steps
    echo "Next steps:"
    echo "  1. Review the patch: less \"$PATCH_FILE\""
    echo "  2. Test applying it: ./apply-dimhouse-patch.sh $ORG_REPO $BRANCH $TARGET_DIR"
    echo "  3. Build with it: ./dimhouse-build.sh -r $ORG_REPO -b $BRANCH"
    echo ""
else
    # In quiet mode, just output the patch file path
    echo "$PATCH_FILE"
fi

# Step 11: Optionally show a preview of the patch
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
            echo "Use 'less \"$PATCH_FILE\"' to view the full patch"
        fi
    fi
fi

if [ "$QUIET" = false ]; then
    echo ""
    echo "✓ Done!"
fi