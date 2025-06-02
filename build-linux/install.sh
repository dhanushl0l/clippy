#!/bin/bash

# Define the URLs for the files :: this is a test scr
URL="https://repo.dhanu.cloud/clippy/clippy-release.tar.xz"
ARCHIVE_NAME="clippy-release.tar.xz"
TARGET_DIR="clippy"

echo "Downloading file..."
curl -L -o "$ARCHIVE_NAME" "$URL"
mkdir -p "$TARGET_DIR"
tar -xJf "$ARCHIVE_NAME" -C "$TARGET_DIR"
cd "$TARGET_DIR" || { echo "Failed to cd into $TARGET_DIR"; exit 1; }

# Define the target locations
BIN_DIR="/usr/local/bin"  
SERVICE_DIR="/etc/systemd/user"   
SERVICE_NAME="clippy.service"

systemctl --user stop "$SERVICE_NAME"

echo "Copying file..."
# Create the service directory if it doesn't exist
mkdir -p "$SERVICE_DIR"

sudo cp  "target/release/clippy"  "$BIN_DIR/clippy"
sudo cp  "target/release/clippy-gui"  "$BIN_DIR/clippy-gui"

sudo cp "build-linux/clippy.service" "$SERVICE_DIR/$SERVICE_NAME"

for f in assets/icons/clippy-*.png; do
  base=$(basename "$f")
  size=$(echo "$base" | sed -E 's/clippy-([0-9]+)-([0-9]+)(@2)?\.png/\1x\2\3/')
  folder=$(echo "$size" | sed 's/@2//')
  target="/usr/share/icons/hicolor/${folder}/apps"
  sudo mkdir -p "$target"
  sudo cp "$f" "$target/clippy.png"
  echo "Copied $f -> $target/clippy.png"
done

sudo cp build-linux/clippy.desktop /usr/share/applications/clippy.desktop

rm -r "&TARGET_DIR"
rm "&ARCHIVE_NAME"

# Reload systemd, enable, and start the service
echo "Enabling and starting the service..."
systemctl --user daemon-reload
systemctl --user enable "$SERVICE_NAME"
systemctl --user start "$SERVICE_NAME"

# Provide feedback
echo "Service has been started and enabled."
