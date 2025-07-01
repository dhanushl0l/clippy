#!/bin/bash

cd "$(dirname "$0")"

URL="https://repo.dhanu.cloud/clippy/clippy-release.tar.xz"
ARCHIVE_NAME="clippy-release.tar.xz"
TARGET_DIR="clippy"
SOURCE_DIR="target/release"
BUILD_LINUX="build-linux"
BUILD_ASSETS="assets"

do_r=false
do_d=false

while getopts "rd" opt; do
  case $opt in
  r) do_r=true ;;
  d) do_d=true ;;
  \?)
    echo "Invalid option: -$OPTARG" >&2
    exit 1
    ;;
  esac
done
if $do_r; then
  SOURCE_DIR="../target/release"
  BUILD_LINUX="../build-linux"
  BUILD_ASSETS="../assets"
elif $do_d; then
  SOURCE_DIR="../target/debug"
  BUILD_LINUX="../build-linux"
  BUILD_ASSETS="../assets"
else
  echo "Downloading file..."
  curl -L -o "$ARCHIVE_NAME" "$URL"
  mkdir -p "$TARGET_DIR"
  tar -xJf "$ARCHIVE_NAME" -C "$TARGET_DIR"
  cd "$TARGET_DIR" || {
    echo "Failed to cd into $TARGET_DIR"
    exit 1
  }
fi

# Define the target locations
BIN_DIR="/usr/local/bin"
SERVICE_DIR="/etc/systemd/user"
SERVICE_NAME="clippy.service"

systemctl --user stop "$SERVICE_NAME"

echo "Copying file..."
# Create the service directory if it doesn't exist
mkdir -p "$SERVICE_DIR"

sudo cp "$SOURCE_DIR/clippy" "$BIN_DIR/clippy"
sudo cp "$SOURCE_DIR/clippy-gui" "$BIN_DIR/clippy-gui"

sudo cp "$BUILD_LINUX/clippy.service" "$SERVICE_DIR/$SERVICE_NAME"

for f in "$BUILD_ASSETS"/icons/clippy-*.png; do
  [[ -e "$f" ]] || continue # skip if no file matched

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
