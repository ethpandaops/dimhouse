#!/bin/bash

set -e

# This script applies the dimhouse patches + overlay to an upstream lighthouse clone
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
TARGET_DIR="${3:-lighthouse}"

# Get the script's directory and repo root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

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

# Find patch file
PATCH_FILE="$REPO_ROOT/patches/$ORG/$REPO/$BRANCH.patch"
if [ ! -f "$PATCH_FILE" ]; then
    echo "Error: Patch not found at patches/$ORG/$REPO/$BRANCH.patch"
    exit 1
fi

echo "Patch file: $(basename "$PATCH_FILE")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Source the shared patch validation function
source "$REPO_ROOT/scripts/validate-patch.sh"

# Validate patch file structure
echo ""
echo -e "${BLUE}=== Applying patch: $(basename "$PATCH_FILE") ===${NC}"
echo -e "${BLUE}  Validating patch file structure...${NC}"
if ! validate_patch_file "$PATCH_FILE"; then
    echo -e "${RED}Patch file is corrupt or malformed${NC}"
    echo -e "${RED}  Please regenerate the patch using save-patch.sh${NC}"
    exit 1
fi
echo -e "${GREEN}  Patch file structure is valid${NC}"

# Apply patch
PATCH_APPLIED=false
if git apply --check "$PATCH_FILE" 2>/dev/null; then
    echo -e "${GREEN}  Patch check passed, applying...${NC}"
    git apply "$PATCH_FILE"
    echo -e "${GREEN}  Patch applied successfully${NC}"
    PATCH_APPLIED=true
elif git apply --check --reverse "$PATCH_FILE" 2>/dev/null; then
    echo -e "${GREEN}  Patch is already applied (verified by reverse check)${NC}"
else
    # Try 3-way merge
    echo -e "${YELLOW}  Direct apply failed, trying 3-way merge...${NC}"
    if git apply --3way "$PATCH_FILE" 2>/dev/null; then
        echo -e "${GREEN}  Patch applied with 3-way merge${NC}"
        PATCH_APPLIED=true
    else
        echo -e "${RED}  Failed to apply patch: $(basename "$PATCH_FILE")${NC}"
        echo ""
        echo "Attempting apply with rejects for diagnostics..."
        git apply --reject "$PATCH_FILE" 2>&1 || true

        REJECT_FILES=$(find . -name "*.rej" 2>/dev/null | sort)
        if [ -n "$REJECT_FILES" ]; then
            echo ""
            echo -e "${YELLOW}=== Patch Conflict Details ===${NC}"
            for reject in $REJECT_FILES; do
                echo -e "${RED}  Conflict in: ${reject%.rej}${NC}"
                cat "$reject" | sed 's/^/    /'
                echo ""
            done
            find . -name "*.rej" -delete 2>/dev/null
            find . -name "*.orig" -delete 2>/dev/null
        fi

        echo -e "${RED}Common causes:${NC}"
        echo "  - The target branch has diverged from when patch was created"
        echo "  - Recent commits modified the same lines"
        echo ""
        CURRENT_BRANCH=$(git branch --show-current 2>/dev/null || echo "detached")
        LATEST_COMMIT=$(git log -1 --oneline)
        echo -e "${BLUE}Current state:${NC}"
        echo "  Branch: $CURRENT_BRANCH"
        echo "  Latest: $LATEST_COMMIT"
        exit 1
    fi
fi

# Copy overlay files
echo ""
echo -e "${BLUE}=== Copying overlay files ===${NC}"

if [ ! -d "$REPO_ROOT/overlay/xatu" ]; then
    echo -e "${RED}Error: xatu overlay not found at $REPO_ROOT/overlay/xatu${NC}"
    exit 1
fi

if [ -d "xatu" ]; then
    echo -e "${YELLOW}  xatu/ directory already exists, replacing...${NC}"
    rm -rf xatu
fi

cp -r "$REPO_ROOT/overlay/xatu" .
echo -e "${GREEN}  Copied overlay/xatu/${NC}"

# Copy CI files
echo ""
echo -e "${BLUE}=== Copying CI files ===${NC}"

if [ -f "$REPO_ROOT/ci/Dockerfile.ethpandaops" ]; then
    cp "$REPO_ROOT/ci/Dockerfile.ethpandaops" Dockerfile
    echo -e "${GREEN}  Copied Dockerfile.ethpandaops -> Dockerfile${NC}"
fi

# Update dependencies
echo ""
echo -e "${BLUE}=== Updating dependencies ===${NC}"
"$REPO_ROOT/scripts/update-deps.sh"

# Append .gitignore entries for xatu build artifacts
echo ""
echo -e "${BLUE}=== Updating .gitignore ===${NC}"
if ! grep -q '/xatu/src/libxatu.so' .gitignore 2>/dev/null; then
    echo "" >> .gitignore
    echo "# Xatu build artifacts" >> .gitignore
    echo "/xatu/src/libxatu.so" >> .gitignore
    echo "/xatu/src/libxatu.h" >> .gitignore
    echo -e "${GREEN}  Added xatu build artifact entries to .gitignore${NC}"
else
    echo -e "${GREEN}  .gitignore already has xatu entries${NC}"
fi

# Disable upstream workflows
echo ""
echo -e "${BLUE}=== Disabling upstream workflows ===${NC}"
"$REPO_ROOT/ci/disable-upstream-workflows.sh"

# Final summary
echo ""
if [ "$PATCH_APPLIED" = false ]; then
    echo -e "${YELLOW}=========================================${NC}"
    echo -e "${YELLOW}  Dimhouse patch was already applied${NC}"
    echo -e "${YELLOW}=========================================${NC}"
else
    echo -e "${GREEN}=========================================${NC}"
    echo -e "${GREEN}  Successfully applied dimhouse patch!${NC}"
    echo -e "${GREEN}=========================================${NC}"
fi
echo -e "  ${GREEN}Overlay files copied${NC}"
echo -e "  ${GREEN}Dependencies updated${NC}"
echo -e "  ${GREEN}Upstream workflows disabled${NC}"
