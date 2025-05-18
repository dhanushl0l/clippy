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

echo "Copying file..."
# Create the service directory if it doesn't exist
mkdir -p "$SERVICE_DIR"
sudo cp  "clippy"  "$BIN_DIR/clippy"
sudo cp  "clippy-gui"  "$BIN_DIR/clippy-gui"
sudo cp "clippy.service" "$SERVICE_DIR/$SERVICE_NAME"
sudo cp assets/clippy-32-32.png /usr/share/icons/hicolor/32x32/apps/clippy.png
sudo cp assets/clippy-512-512.png /usr/share/icons/hicolor/512x512/apps/clippy.png
sudo cp clippy.desktop /usr/share/applications/clippy.desktop

cd ..
rm -r "&TARGET_DIR"
rm "&ARCHIVE_NAME"

# Reload systemd, enable, and start the service
echo "Enabling and starting the service..."
systemctl --user daemon-reload
systemctl --user enable "$SERVICE_NAME"
systemctl --user start "$SERVICE_NAME"

# Provide feedback
echo "Service has been started and enabled."
