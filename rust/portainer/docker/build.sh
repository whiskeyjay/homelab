#!/bin/bash

# You need to be logged in to Docker Hub before running this script.

# Version configuration
VERSION="0.1.4"
MINOR_VERSION="0.1"
IMAGE_NAME="whiskeyjay/ptn2influx"

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

cd "$SCRIPT_DIR/../.."
cp "target/$RUST_TARGET/release/ptn2influx" "$BUILD_DIR/bin/linux/$DOCKER_ARCH/"

cp "$SCRIPT_DIR/dockerfile" "$BUILD_DIR"
cp "$SCRIPT_DIR/entrypoint.sh" "$BUILD_DIR"

cd "$BUILD_DIR"

# Build and push with platform-specific tags
docker build \
    -t $IMAGE_NAME:$VERSION-$DOCKER_ARCH \
    -t $IMAGE_NAME:$MINOR_VERSION-$DOCKER_ARCH \
    .

docker push $IMAGE_NAME:$VERSION-$DOCKER_ARCH
docker push $IMAGE_NAME:$MINOR_VERSION-$DOCKER_ARCH

echo ""
echo "============================================"
echo "Image pushed: $IMAGE_NAME:$VERSION-$DOCKER_ARCH"
echo "============================================"
echo ""
echo "After building on BOTH architectures, run this command to create the multi-arch manifest:"
echo ""
echo "docker manifest create $IMAGE_NAME:latest \\"
echo "  $IMAGE_NAME:$VERSION-amd64 \\"
echo "  $IMAGE_NAME:$VERSION-arm64"
echo ""
echo "docker manifest create $IMAGE_NAME:$VERSION \\"
echo "  $IMAGE_NAME:$VERSION-amd64 \\"
echo "  $IMAGE_NAME:$VERSION-arm64"
echo ""
echo "docker manifest create $IMAGE_NAME:$MINOR_VERSION \\"
echo "  $IMAGE_NAME:$VERSION-amd64 \\"
echo "  $IMAGE_NAME:$VERSION-arm64"
echo ""
echo "docker manifest push $IMAGE_NAME:latest"
echo "docker manifest push $IMAGE_NAME:$VERSION"
echo "docker manifest push $IMAGE_NAME:$MINOR_VERSION"
echo ""
