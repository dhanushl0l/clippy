#!/bin/bash

NAME=clippy
VIRSION=$(grep '^version' clippy/Cargo.toml | head -n1 | cut -d'"' -f2)
FILES=("build-linux/clippy.desktop" "build-linux/clippy.service")
FILES_RELEASE=("target/release/clippy-gui" "target/release/clippy")
FILES_DEBUG=("target/debug/clippy-gui" "target/debug/clippy")
CONFIG_DIR="config"
ARCH=$(uname -m)

rm temp/*
mkdir -p temp
mkdir -p "temp/${CONFIG_DIR}"
for file in "${FILES[@]}"; do
	cp "$file" "temp/${CONFIG_DIR}/"
done
cp -r assets temp/assets
while getopts "rd" opt; do
	case "$opt" in
	r)
		MODE="release"
		;;
	d)
		MODE="debug"
		;;
	*)
		echo "Usage: $0 [-r] [-d]"
		MODE="debug"
		;;
	esac
done

echo "Building version $VERSION"

mkdir -p "temp/target"
KEY=$(head -c 32 /dev/urandom | base64 | head -c 32)
if [ "$MODE" == "release" ]; then
	KEY="$KEY" cargo build --release --bin clippy --bin clippy-gui
	cp "${FILES_RELEASE[0]}" "temp/target/"
	cp "${FILES_RELEASE[1]}" "temp/target/"
else
	KEY="$KEY" cargo build --bin clippy --bin clippy-gui
	cp "${FILES_DEBUG[0]}" "temp/target/"
	cp "${FILES_DEBUG[1]}" "temp/target/"

fi

mkdir -p build
tar -cJf "build/${NAME}-${VIRSION}-linux-${ARCH}.tar.xz" -C temp .
rm -rf temp
