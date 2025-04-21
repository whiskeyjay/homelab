#!/bin/bash

# You need to be logged in to Docker Hub before running this script.

# Script directory
SCRIPT_DIR=$(dirname "$(realpath "$0")")

cargo clean
cargo build --release --target x86_64-unknown-linux-musl
cargo build --release --target aarch64-unknown-linux-musl

# Temporary directory for build
BUILD_DIR="$SCRIPT_DIR/.temp"

if [ -d "$BUILD_DIR" ]; then
    rm -rf "$BUILD_DIR"
fi
mkdir -p "$BUILD_DIR"
mkdir -p "$BUILD_DIR/bin/linux/amd64"
mkdir -p "$BUILD_DIR/bin/linux/arm64"

cd "$SCRIPT_DIR/.."
cp target/x86_64-unknown-linux-musl/release/ptn2influx "$BUILD_DIR/bin/linux/amd64/"
cp target/aarch64-unknown-linux-musl/release/ptn2influx "$BUILD_DIR/bin/linux/arm64/"

cp "$SCRIPT_DIR/dockerfile" "$BUILD_DIR"
cp "$SCRIPT_DIR/entrypoint.sh" "$BUILD_DIR"

cd "$BUILD_DIR"

docker buildx build \
    -t whiskeyjay/ptn2influx:latest \
    -t whiskeyjay/ptn2influx:0.1.1 \
    -t whiskeyjay/ptn2influx:0.1 \
    --platform linux/amd64,linux/arm64 \
    --push \
    .
