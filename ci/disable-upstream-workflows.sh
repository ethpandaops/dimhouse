#!/bin/bash
# Disable all upstream GitHub Actions workflows by renaming them to .disabled
# Our dimhouse-* workflows are preserved.
set -e

WORKFLOW_DIR="${1:-.github/workflows}"

if [ ! -d "$WORKFLOW_DIR" ]; then
    echo "No workflows directory found at $WORKFLOW_DIR"
    exit 0
fi

DISABLED=0
for f in "$WORKFLOW_DIR"/*.yml "$WORKFLOW_DIR"/*.yaml; do
    [ -f "$f" ] || continue
    name="$(basename "$f")"
    case "$name" in
        ethpandaops-*|dimhouse-*) ;; # Keep our workflows
        *) mv "$f" "${f}.disabled"; DISABLED=$((DISABLED + 1)) ;;
    esac
done

echo "Disabled $DISABLED upstream workflow(s)"
