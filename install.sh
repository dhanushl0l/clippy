#!/bin/bash

# Define the URLs for the files :: this is a test scr
BIN_URL="target/release/clippy"
SERVICE_URL="clippy.service"

# Define the target locations
BIN_DIR="/usr/local/bin"  
SERVICE_DIR="/etc/systemd/user"   
SERVICE_NAME="clippy.service"

# Create the service directory if it doesn't exist
mkdir -p "$SERVICE_DIR"

# Download the binary file and the service file
echo "Downloading binary file..."
sudo cp  "$BIN_URL"  "$BIN_DIR/clippy"

echo "Downloading service file..."
sudo cp "$SERVICE_URL" "$SERVICE_DIR/$SERVICE_NAME"

# Make the binary file executable
chmod +x "$BIN_DIR/clippy"

# Reload systemd, enable, and start the service
echo "Enabling and starting the service..."
systemctl --user daemon-reload
systemctl --user enable "$SERVICE_NAME"
systemctl --user start "$SERVICE_NAME"

# Provide feedback
echo "Service has been started and enabled."
