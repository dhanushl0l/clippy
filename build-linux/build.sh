#!/bin/bash

# Defaults
MODE="release"
NAME="clippy-release.tar.xz"
KEY="$KEY"

# Files to include
FILES=("build-linux/clippy.desktop" "build-linux/clippy.service")
FILES_RELEASE=("target/release/clippy-gui" "target/release/clippy")
FILES_DEBUG=("target/debug/clippy-gui" "target/debug/clippy")
DIR=("assets")

# Parse flags
while getopts "rvd" opt; do
  case "$opt" in
    r) MODE="release"; NAME="clippy-release.tar.xz" ;;
    v) MODE="version" ;;
    d) MODE="debug"; NAME="clippy-debug.tar.xz" ;;
    *) echo "Usage: $0 [-r] [-v] [-d]"; exit 1 ;;
  esac
done

# Ask for version if -v
if [ "$MODE" == "version" ]; then
  read -p "Enter version: " VER
  NAME="clippy-$VER.tar.xz"
  MODE="release"
fi

# Build
if [ "$MODE" == "debug" ]; then
  KEY="$KEY" cargo build
else
  KEY="$KEY" cargo build --release
fi

# Collect files based on mode
INCLUDE_FILES=("${FILES[@]}" "${DIR[@]}")
if [ "$MODE" == "debug" ]; then
  INCLUDE_FILES+=("${FILES_DEBUG[@]}")
else
  INCLUDE_FILES+=("${FILES_RELEASE[@]}")
fi

# Create archive
tar -cJf "$NAME" "${INCLUDE_FILES[@]}"

echo "Created archive: $NAME"
