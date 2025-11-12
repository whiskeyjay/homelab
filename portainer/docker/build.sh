#!/bin/bash

# You need to be logged in to Docker Hub before running this script.

# Script directory
SCRIPT_DIR=$(dirname "$(realpath "$0")")

# Auto-detect CPU architecture
ARCH=$(uname -m)

case "$ARCH" in
    x86_64)
        RUST_TARGET="x86_64-unknown-linux-gnu"
        DOCKER_PLATFORM="linux/amd64"
        DOCKER_ARCH="amd64"
        echo "Detected x86_64 architecture"
        ;;
    aarch64|arm64)
        RUST_TARGET="aarch64-unknown-linux-gnu"
        DOCKER_PLATFORM="linux/arm64"
        DOCKER_ARCH="arm64"
        echo "Detected ARM64 architecture"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

cargo clean
cargo build --release --target "$RUST_TARGET"

# Temporary directory for build
BUILD_DIR="$SCRIPT_DIR/.temp"

if [ -d "$BUILD_DIR" ]; then
    rm -rf "$BUILD_DIR"
fi
mkdir -p "$BUILD_DIR"
mkdir -p "$BUILD_DIR/bin/linux/$DOCKER_ARCH"

cd "$SCRIPT_DIR/.."
cp "target/$RUST_TARGET/release/ptn2influx" "$BUILD_DIR/bin/linux/$DOCKER_ARCH/"

cp "$SCRIPT_DIR/dockerfile" "$BUILD_DIR"
cp "$SCRIPT_DIR/entrypoint.sh" "$BUILD_DIR"

cd "$BUILD_DIR"

docker buildx build \
    -t whiskeyjay/ptn2influx:latest \
    -t whiskeyjay/ptn2influx:0.1.3 \
    -t whiskeyjay/ptn2influx:0.1 \
    --platform "$DOCKER_PLATFORM" \
    --push \
    .
