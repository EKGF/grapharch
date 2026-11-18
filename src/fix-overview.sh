#!/bin/bash
# Script to update the section index to make the overview chapter appear first
# Usage: ./fix-overview.sh [path to typst_debug directory]

TYPST_DIR=${1:-"typst_debug"}
INDEX_FILE="$TYPST_DIR/dprod/section-1/index.typ"

if [ ! -f "$INDEX_FILE" ]; then
    echo "Error: Index file not found at $INDEX_FILE"
    exit 1
fi

if [ ! -d "$TYPST_DIR/dprod/section-1/chapter-0" ]; then
    # If chapter-0 doesn't exist yet, create it by copying chapter-7
    mkdir -p "$TYPST_DIR/dprod/section-1/chapter-0"
    if [ -f "$TYPST_DIR/dprod/section-1/chapter-7/index.typ" ]; then
        cp "$TYPST_DIR/dprod/section-1/chapter-7/index.typ" "$TYPST_DIR/dprod/section-1/chapter-0/index.typ"
    fi
fi

# Read the current content of the index file
content=$(cat "$INDEX_FILE")

# Check if chapter-0 is already included
if ! grep -q 'include "chapter-0/index.typ"' "$INDEX_FILE"; then
    # Add chapter-0 after the header
    new_content=$(echo "$content" | awk '
        /This section contains documentation/ {
            print;
            print "";
            print "#include \"chapter-0/index.typ\"";
            next;
        }
        {print}
    ')
    echo "$new_content" > "$INDEX_FILE"
    echo "Updated $INDEX_FILE to include chapter-0 first"
else
    echo "chapter-0 is already included in $INDEX_FILE"
fi

# Run the typst compiler to regenerate the PDF
echo "Running typst compiler to regenerate the PDF"
