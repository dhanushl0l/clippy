#!/bin/bash

cd "$(dirname "$0")"

ARCH=$(uname -m)
VERSION="0.1.5"
TARGET_DIR="clippy"
SOURCE_DIR="target"
BUILD_ASSETS="assets"

# Define the target locations
BIN_DIR="/usr/local/bin"
SERVICE_DIR="/etc/systemd/user"
SERVICE_NAME="clippy.service"
BUILD_LINUX="config"

while getopts "rd" opt; do
	case "$opt" in
	r)
		TYPE="release"
		;;
	d)
		TYPE="debug"
		;;
	*)
		TYPE="release"
		;;
	esac
done

URL="https://github.com/dhanushl0l/clippy/releases/download/v$VERSION/clippy-$VERSION-linux-$ARCH.tar.xz"
ARCHIVE_NAME="clippy-release.tar.xz"

curl -L "$URL" -o "$ARCHIVE_NAME"
mkdir $TARGET_DIR
tar -xJf $ARCHIVE_NAME -C $TARGET_DIR
cd $TARGET_DIR

if systemctl --user is-active --quiet "$SERVICE_NAME"; then
	echo "Hey! You're updating $SERVICE_NAME..."
	systemctl --user stop "$SERVICE_NAME"
else
	echo "Installing clippy"
fi

echo "Copying file..."
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

echo "Service has been started and enabled."

cd ..

rm $ARCHIVE_NAME
rm -r $TARGET_DIR
