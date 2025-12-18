#!/bin/bash
# Validates that a patch file is not corrupt or malformed
# Usage: ./validate-patch.sh <patch_file>
# Exit codes: 0 = valid, 1 = invalid

set -e

RED='\033[0;31m'
NC='\033[0m'

validate_patch_file() {
    local patch_file="$1"
    local errors=()

    # Check file exists and is readable
    if [ ! -r "$patch_file" ]; then
        echo "Patch file not readable: $patch_file"
        return 1
    fi

    # Check file is not empty
    if [ ! -s "$patch_file" ]; then
        echo "Patch file is empty: $patch_file"
        return 1
    fi

    # Check file ends with a newline (common corruption issue)
    if [ "$(tail -c 1 "$patch_file" | xxd -p)" != "0a" ]; then
        errors+=("Missing newline at end of file")
    fi

    # Validate each hunk header matches content
    local in_hunk=false
    local expected_old=0
    local expected_new=0
    local actual_old=0
    local actual_new=0
    local hunk_start_line=0
    local line_num=0

    while IFS= read -r line || [ -n "$line" ]; do
        line_num=$((line_num + 1))

        # Check for hunk header
        if [[ "$line" =~ ^@@[[:space:]]-([0-9]+),([0-9]+)[[:space:]]\+([0-9]+),([0-9]+)[[:space:]]@@.* ]]; then
            # Validate previous hunk if there was one
            if [ "$in_hunk" = true ]; then
                if [ "$actual_old" -ne "$expected_old" ]; then
                    errors+=("Hunk at line $hunk_start_line: expected $expected_old old lines, got $actual_old")
                fi
                if [ "$actual_new" -ne "$expected_new" ]; then
                    errors+=("Hunk at line $hunk_start_line: expected $expected_new new lines, got $actual_new")
                fi
            fi

            # Start new hunk
            in_hunk=true
            hunk_start_line=$line_num
            expected_old="${BASH_REMATCH[2]}"
            expected_new="${BASH_REMATCH[4]}"
            actual_old=0
            actual_new=0
        elif [ "$in_hunk" = true ]; then
            # Count lines in hunk
            if [[ "$line" =~ ^[[:space:]] ]] || [[ "$line" == "" ]]; then
                # Context line (counts for both)
                actual_old=$((actual_old + 1))
                actual_new=$((actual_new + 1))
            elif [[ "$line" =~ ^\+ ]]; then
                # Added line
                actual_new=$((actual_new + 1))
            elif [[ "$line" =~ ^- ]]; then
                # Removed line
                actual_old=$((actual_old + 1))
            elif [[ "$line" =~ ^diff|^index|^---|^\+\+\+ ]]; then
                # New file header - validate previous hunk
                if [ "$actual_old" -ne "$expected_old" ]; then
                    errors+=("Hunk at line $hunk_start_line: expected $expected_old old lines, got $actual_old")
                fi
                if [ "$actual_new" -ne "$expected_new" ]; then
                    errors+=("Hunk at line $hunk_start_line: expected $expected_new new lines, got $actual_new")
                fi
                in_hunk=false
            fi
        fi
    done < "$patch_file"

    # Validate final hunk
    if [ "$in_hunk" = true ]; then
        if [ "$actual_old" -ne "$expected_old" ]; then
            errors+=("Hunk at line $hunk_start_line: expected $expected_old old lines, got $actual_old")
        fi
        if [ "$actual_new" -ne "$expected_new" ]; then
            errors+=("Hunk at line $hunk_start_line: expected $expected_new new lines, got $actual_new")
        fi
    fi

    if [ ${#errors[@]} -gt 0 ]; then
        echo -e "${RED}Patch file validation failed: $(basename "$patch_file")${NC}"
        for err in "${errors[@]}"; do
            echo -e "${RED}  - $err${NC}"
        done
        return 1
    fi

    return 0
}

# If run directly (not sourced), validate the provided file
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    if [ $# -ne 1 ]; then
        echo "Usage: $0 <patch_file>"
        exit 1
    fi
    validate_patch_file "$1"
fi
