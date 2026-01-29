#!/bin/bash
# Script to prepare DMG support files before Tauri builds the DMG
# This copies the create-dmg support files to the bundle directory

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
SRC_TAURI="$REPO_ROOT/src-tauri"

# Determine the target directory based on build profile
if [[ "$1" == "--release" ]] || [[ -z "$1" ]]; then
    TARGET_DIR="$SRC_TAURI/target/release/bundle/dmg"
else
    TARGET_DIR="$SRC_TAURI/target/debug/bundle/dmg"
fi

# Create the target directory if it doesn't exist
mkdir -p "$TARGET_DIR"

# Copy the support files
echo "Copying DMG support files to $TARGET_DIR..."
cp -r "$SRC_TAURI/dmg-support/support" "$TARGET_DIR/"
cp "$SRC_TAURI/dmg-support/.this-is-the-create-dmg-repo" "$TARGET_DIR/"

echo "DMG support files copied successfully."
