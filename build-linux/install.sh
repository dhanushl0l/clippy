#!/bin/bash

cd "$(dirname "$0")"

URL="https://repo.dhanu.cloud/clippy/clippy-release.tar.xz"
ARCHIVE_NAME="clippy-release.tar.xz"
TARGET_DIR="clippy"
SOURCE_DIR="target"
BUILD_LINUX="config"
BUILD_ASSETS="assets"

while getopts "rd" opt; do
  case "$opt" in
  r)
    BUILD_LINUX="build-linux"
    ;;
  d)
    BUILD_LINUX="build-linux"
    ;;
  *)
    echo "Usage: $0 [-r] [-d]"
    exit 1
    ;;
  esac
done

# Define the target locations
BIN_DIR="/usr/local/bin"
SERVICE_DIR="/etc/systemd/user"
SERVICE_NAME="clippy.service"

if systemctl --user is-active --quiet "$SERVICE_NAME"; then
  echo "👋 Hey! You're updating $SERVICE_NAME..."
  systemctl --user stop "$SERVICE_NAME"
else
  echo "Installing clippy"
fi

echo "Copying file..."
# Create the service directory if it doesn't exist
mkdir -p "$SERVICE_DIR"

sudo cp "$SOURCE_DIR/clippy" "$BIN_DIR/clippy"
sudo cp "$SOURCE_DIR/clippy-gui" "$BIN_DIR/clippy-gui"
sudo cp "$BUILD_LINUX/clippy.service" "$SERVICE_DIR/$SERVICE_NAME"

for f in "$BUILD_ASSETS"/icons/clippy-*.png; do
  [[ -e "$f" ]] || continue

  base=$(basename "$f")
  size=$(echo "$base" | sed -E 's/clippy-([0-9]+)-([0-9]+)(@2)?\.png/\1x\2/')
  folder=$(echo "$size" | sed 's/@2//')
  target="/usr/share/icons/hicolor/${folder}/apps"
  sudo mkdir -p "$target"
  sudo cp "$f" "$target/clippy.png"
  echo "Copied $f -> $target/clippy.png"
done

sudo cp "$BUILD_LINUX/clippy.desktop" "/usr/share/applications/clippy.desktop"

if ! $do_d && ! $do_r; then
  echo "Deleting temp files..."
  cd ..
  rm -r "$TARGET_DIR"
  rm "$ARCHIVE_NAME"
fi

# Reload systemd, enable, and start the service
echo "Enabling and starting the service..."
systemctl --user daemon-reload
systemctl --user enable "$SERVICE_NAME"
systemctl --user start "$SERVICE_NAME"

# Provide feedback
echo "Service has been started and enabled."
