#!/bin/bash

# dimhouse-build.sh - Clone upstream lighthouse, apply patches + overlay, and build
# Usage: ./dimhouse-build.sh -r <org/repo> -b <branch> [-c <commit>] [--ci] [--skip-build]

set -e

# Default values
ORG=""
REPO=""
BRANCH=""
COMMIT=""
CI_MODE=false
SKIP_BUILD=false

# Parse arguments
while [ $# -gt 0 ]; do
    case "$1" in
        -r|--repo)
            IFS='/' read -ra REPO_PARTS <<< "$2"
            if [ ${#REPO_PARTS[@]} -ne 2 ]; then
                echo "Error: Repository must be in format 'org/repo'"
                exit 1
            fi
            ORG="${REPO_PARTS[0]}"
            REPO="${REPO_PARTS[1]}"
            shift 2
            ;;
        -b|--branch)
            BRANCH="$2"
            shift 2
            ;;
        -c|--commit)
            COMMIT="$2"
            shift 2
            ;;
        --ci)
            CI_MODE=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 -r org/repo -b branch [-c commit] [--ci] [--skip-build]"
            echo "  -c, --commit: Pin to specific commit SHA (optional)"
            echo "  --ci: Run in CI mode (non-interactive, auto-clean, auto-update patches)"
            echo "  --skip-build: Skip the build step (useful for Docker builds)"
            exit 1
            ;;
    esac
done

# Validate required arguments
if [ -z "$ORG" ] || [ -z "$REPO" ] || [ -z "$BRANCH" ]; then
    echo "Error: Missing required arguments"
    echo "Usage: $0 -r org/repo -b branch [-c commit] [--ci] [--skip-build]"
    echo "Example: $0 -r sigp/lighthouse -b unstable"
    echo "Example: $0 -r sigp/lighthouse -b unstable -c 5aff1fcb75befcde2f956a5b38a9deec5cc4123c"
    exit 1
fi

if [ -n "$COMMIT" ]; then
    echo "Testing with repository: $ORG/$REPO on branch: $BRANCH at commit: $COMMIT"
else
    echo "Testing with repository: $ORG/$REPO on branch: $BRANCH"
fi

# Store the script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check if lighthouse directory exists and handle it
if [ -d "lighthouse" ]; then
    echo "Found existing lighthouse directory, checking..."
    cd lighthouse

    if [ -d ".git" ]; then
        CURRENT_REMOTE=$(git config --get remote.origin.url || echo "")
        EXPECTED_REMOTE="https://github.com/$ORG/$REPO.git"

        if [ "$CURRENT_REMOTE" != "$EXPECTED_REMOTE" ]; then
            echo "Remote mismatch: current=$CURRENT_REMOTE, expected=$EXPECTED_REMOTE"
            cd ..
            echo "Removing existing lighthouse directory..."
            rm -rf lighthouse
            echo "Cloning repository..."
            if [ -n "$COMMIT" ]; then
                git clone --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
                cd lighthouse && git checkout "$COMMIT" && cd ..
            else
                git clone --depth 1 --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
            fi
        else
            CURRENT_BRANCH=$(git branch --show-current)
            if [ "$CURRENT_BRANCH" != "$BRANCH" ]; then
                echo "Branch mismatch: current=$CURRENT_BRANCH, expected=$BRANCH"

                if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
                    if [ "$CI_MODE" = true ]; then
                        echo "CI mode: Auto-cleaning lighthouse directory..."
                        git reset --hard
                        git clean -fd
                    else
                        echo "WARNING: lighthouse directory has uncommitted changes"
                        git status --short | head -20
                        read -p "Clean lighthouse directory before switching branches? (y/N) " -n 1 -r
                        echo ""
                        if [[ $REPLY =~ ^[Yy]$ ]]; then
                            git reset --hard
                            git clean -fd
                        else
                            echo "Cannot switch branches with uncommitted changes. Exiting."
                            cd ..
                            exit 1
                        fi
                    fi
                fi

                echo "Switching to branch $BRANCH..."
                git fetch --depth 1 origin "$BRANCH":"$BRANCH"
                git checkout "$BRANCH"
            fi

            # Update to target (specific commit or latest)
            if [ -n "$COMMIT" ]; then
                echo "Checking for target commit..."
                LOCAL=$(git rev-parse HEAD)
                if [ "$LOCAL" != "$COMMIT" ]; then
                    echo "Fetching and checking out commit $COMMIT..."
                    git fetch origin "$BRANCH"
                    git checkout "$COMMIT"
                else
                    echo "Already on target commit"
                fi
            else
                echo "Checking for updates..."
                git fetch --depth 1 origin "$BRANCH" || true

                if git rev-parse --verify "origin/$BRANCH" >/dev/null 2>&1; then
                    LOCAL=$(git rev-parse HEAD)
                    REMOTE=$(git rev-parse "origin/$BRANCH")

                    if [ "$LOCAL" != "$REMOTE" ]; then
                        echo "Local branch is behind remote, pulling latest..."
                        git pull --depth 1 origin "$BRANCH"
                    else
                        echo "Already on latest commit"
                    fi
                fi
            fi

            cd ..
        fi
    else
        cd ..
        echo "Directory exists but is not a git repository, removing..."
        rm -rf lighthouse
        echo "Cloning repository..."
        if [ -n "$COMMIT" ]; then
            git clone --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
            cd lighthouse && git checkout "$COMMIT" && cd ..
        else
            git clone --depth 1 --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
        fi
    fi
else
    echo "Cloning repository..."
    if [ -n "$COMMIT" ]; then
        git clone --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
        cd lighthouse && git checkout "$COMMIT" && cd ..
    else
        git clone --depth 1 --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
    fi
fi

# Clean lighthouse directory if it has leftover changes
echo "Checking lighthouse directory status..."
cd lighthouse
if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
    echo "WARNING: lighthouse directory has changes"
    if [ "$CI_MODE" = true ]; then
        echo "CI mode: Auto-cleaning..."
        git reset --hard
        git clean -fd
    else
        git status --short | head -20
        read -p "Clean lighthouse directory before continuing? (y/N) " -n 1 -r
        echo ""
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            git reset --hard
            git clean -fd
        fi
    fi
fi
cd ..

# Apply the dimhouse patches + overlay
echo "Applying dimhouse patches..."
"$SCRIPT_DIR/apply-dimhouse-patch.sh" "$ORG/$REPO" "$BRANCH" lighthouse

if [ "$SKIP_BUILD" = true ]; then
    echo ""
    echo "Build skipped (--skip-build). Patched source is in lighthouse/"
    exit 0
fi

# Build the project
echo ""
echo "Building lighthouse..."
cd lighthouse
if cargo build --release; then
    echo "Build completed successfully!"
    cd ..

    echo ""
    echo "Generating patch from build changes..."

    CI_FLAGS=""
    if [ "$CI_MODE" = true ]; then
        CI_FLAGS="--ci"
    fi

    if [ "$CI_MODE" = true ]; then
        PATCH_OUTPUT=$("$SCRIPT_DIR/save-patch.sh" -r "$ORG/$REPO" -b "$BRANCH" $CI_FLAGS lighthouse 2>&1) || PATCH_EXIT_CODE=$?
        PATCH_EXIT_CODE=${PATCH_EXIT_CODE:-0}
    else
        "$SCRIPT_DIR/save-patch.sh" -r "$ORG/$REPO" -b "$BRANCH" lighthouse || PATCH_EXIT_CODE=$?
        PATCH_EXIT_CODE=${PATCH_EXIT_CODE:-0}
    fi

    if [ $PATCH_EXIT_CODE -eq 0 ]; then
        [ "$CI_MODE" = true ] && echo "Patch saved: $PATCH_OUTPUT"
        echo ""
        echo "Build and patch generation completed successfully!"
    elif [ $PATCH_EXIT_CODE -eq 2 ]; then
        echo "No changes detected - patch unchanged"
    else
        echo "Warning: Failed to generate patch (exit code: $PATCH_EXIT_CODE)"
    fi
else
    echo "Build failed!"
    exit 1
fi
