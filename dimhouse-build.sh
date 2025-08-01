#!/bin/bash

set -e

# Default values
ORG=""
REPO=""
BRANCH=""

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
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
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 -r org/repo -b branch"
            exit 1
            ;;
    esac
done

# Validate required arguments
if [ -z "$ORG" ] || [ -z "$REPO" ] || [ -z "$BRANCH" ]; then
    echo "Error: Missing required arguments"
    echo "Usage: $0 -r org/repo -b branch"
    echo "Example: $0 -r sigp/lighthouse -b unstable"
    exit 1
fi

echo "Testing with repository: $ORG/$REPO on branch: $BRANCH"

# Check if lighthouse directory exists and handle it smartly
if [ -d "lighthouse" ]; then
    echo "Found existing lighthouse directory, checking..."
    cd lighthouse
    
    # Check if it's a git repository
    if [ -d ".git" ]; then
        # Get current remote URL
        CURRENT_REMOTE=$(git config --get remote.origin.url || echo "")
        EXPECTED_REMOTE="https://github.com/$ORG/$REPO.git"
        
        # Check if the remote matches
        if [ "$CURRENT_REMOTE" != "$EXPECTED_REMOTE" ]; then
            echo "Remote mismatch: current=$CURRENT_REMOTE, expected=$EXPECTED_REMOTE"
            cd ..
            echo "Removing existing lighthouse directory..."
            rm -rf lighthouse
            echo "Cloning repository..."
            git clone --depth 1 --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
        else
            # Check current branch
            CURRENT_BRANCH=$(git branch --show-current)
            if [ "$CURRENT_BRANCH" != "$BRANCH" ]; then
                echo "Branch mismatch: current=$CURRENT_BRANCH, expected=$BRANCH"
                echo "Switching to branch $BRANCH..."
                git fetch --depth 1 origin "$BRANCH":"$BRANCH"
                git checkout "$BRANCH"
            fi
            
            # Check if we're on the latest commit
            echo "Checking for updates..."
            git fetch --depth 1
            LOCAL=$(git rev-parse HEAD)
            REMOTE=$(git rev-parse origin/"$BRANCH")
            
            if [ "$LOCAL" != "$REMOTE" ]; then
                echo "Local branch is behind remote, pulling latest changes..."
                git pull --depth 1
            else
                echo "Already on latest commit"
            fi
            
            cd ..
        fi
    else
        # Not a git repository
        cd ..
        echo "Directory exists but is not a git repository, removing..."
        rm -rf lighthouse
        echo "Cloning repository..."
        git clone --depth 1 --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
    fi
else
    # No lighthouse directory exists
    echo "Cloning repository..."
    git clone --depth 1 --branch "$BRANCH" "https://github.com/$ORG/$REPO.git" lighthouse
fi

# Check if diff file exists, otherwise use default
DIFF_FILE="diffs/$ORG/$REPO/$BRANCH.diff"
if [ ! -f "$DIFF_FILE" ]; then
    echo "Diff file not found at $DIFF_FILE, using default..."
    DIFF_FILE="diffs/sigp/lighthouse/unstable.diff"
fi

if [ ! -f "$DIFF_FILE" ]; then
    echo "Error: No diff file found"
    exit 1
fi

# Check if lighthouse directory is dirty
echo "Checking lighthouse directory status..."
cd lighthouse
if [ -d "xatu" ] || ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
    echo "WARNING: Lighthouse directory has changes:"
    # Show xatu directory if it exists
    if [ -d "xatu" ]; then
        echo "  - xatu/ directory exists"
    fi
    # Show git status if there are changes
    if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
        git status --short | head -20
        if [ $(git status --short | wc -l) -gt 20 ]; then
            echo "  ... and more files"
        fi
    fi
    echo ""
    read -p "Clean lighthouse directory before continuing? (y/N) " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Cleaning lighthouse directory..."
        rm -rf xatu
        git reset --hard
        echo "Clean complete."
    else
        echo "Continuing without cleaning..."
    fi
fi
cd ..

echo "Applying diff from $DIFF_FILE..."
cd lighthouse
git apply ../"$DIFF_FILE"
cd ..

# Copy the xatu crate
echo "Copying xatu crate..."
cp -r crates/xatu lighthouse/

# Build the project
echo "Building lighthouse..."
cd lighthouse
if cargo build --release; then
    echo "Build completed successfully!"
    
    # Reset Cargo.lock to original state before creating diff
    echo "Resetting Cargo.lock to original state..."
    git checkout Cargo.lock
    
    # Generate new diff
    echo "Generating new diff..."
    # Exclude xatu directory from the diff
    rm -rf xatu
    git diff --cached > ../new.diff
    git reset
    cd ..
    
    # Ensure target directory exists
    TARGET_DIFF_DIR="diffs/$ORG/$REPO"
    mkdir -p "$TARGET_DIFF_DIR"
    
    TARGET_DIFF_FILE="$TARGET_DIFF_DIR/$BRANCH.diff"
    
    # Compare diffs and update if different
    if [ -f "$DIFF_FILE" ]; then
        if ! diff -q "$DIFF_FILE" new.diff > /dev/null 2>&1; then
            echo "Diff has changed from the original."
            echo ""
            read -p "Update the diff file at $TARGET_DIFF_FILE? (y/N) " -n 1 -r
            echo ""
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                echo "Updating $TARGET_DIFF_FILE..."
                cp new.diff "$TARGET_DIFF_FILE"
                echo "Diff updated successfully!"
            else
                echo "Keeping original diff unchanged."
            fi
        else
            echo "Diff has not changed."
        fi
    else
        # First time creating this diff
        echo "Creating new diff at $TARGET_DIFF_FILE..."
        cp new.diff "$TARGET_DIFF_FILE"
    fi
    
    # Clean up temporary files
    rm -f new.diff
else
    echo "Build failed!"
    exit 1
fi