#!/bin/bash

# You need to be logged in to Docker Hub before running this script.
# Usage: build.sh [amd64|arm64]  (defaults to the host architecture)
# Building a non-host architecture needs QEMU: docker run --privileged --rm tonistiigi/binfmt --install arm64

set -e

# Version configuration - read from Cargo.toml
SCRIPT_DIR=$(dirname "$(realpath "$0")")
VERSION=$(grep '^version' "$SCRIPT_DIR/../Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')
MINOR_VERSION=$(echo "$VERSION" | sed 's/\.[^.]*$//')
IMAGE_NAME="whiskeyjay/doh-proxy"

echo "Version: $VERSION (minor: $MINOR_VERSION)"

# Target CPU architecture: first argument, or auto-detected from the host
ARCH=${1:-$(uname -m)}

case "$ARCH" in
    x86_64|amd64)
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

# Navigate to workspace root (rust folder)
WORKSPACE_ROOT="$SCRIPT_DIR/../.."
cd "$WORKSPACE_ROOT"

if [ "$DOCKER_PLATFORM" = "$(docker version -f '{{.Server.Os}}/{{.Server.Arch}}')" ]; then
    cargo build --release --target "$RUST_TARGET"
else
    echo "Cross-building $RUST_TARGET under QEMU emulation (slow)"
    docker run --rm --pull always --platform "$DOCKER_PLATFORM" \
        -u "$(id -u):$(id -g)" \
        -e CARGO_HOME="/src/target/.cargo-$DOCKER_ARCH" \
        -v "$PWD":/src -w /src \
        rust:1-trixie \
        cargo build --release --target "$RUST_TARGET" -p doh-proxy
fi

# Temporary directory for build
BUILD_DIR="$SCRIPT_DIR/.temp"

if [ -d "$BUILD_DIR" ]; then
    rm -rf "$BUILD_DIR"
fi
mkdir -p "$BUILD_DIR"
mkdir -p "$BUILD_DIR/bin/linux/$DOCKER_ARCH"

# Copy binary from workspace target directory
cp "target/$RUST_TARGET/release/doh-proxy" "$BUILD_DIR/bin/linux/$DOCKER_ARCH/"

cp "$SCRIPT_DIR/dockerfile" "$BUILD_DIR"
cp "$SCRIPT_DIR/entrypoint.sh" "$BUILD_DIR"

cd "$BUILD_DIR"

# Build and push with platform-specific tags
docker build \
    --platform "$DOCKER_PLATFORM" \
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
