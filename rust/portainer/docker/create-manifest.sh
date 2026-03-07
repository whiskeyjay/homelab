#!/bin/bash

# This script creates and pushes multi-arch manifests after building on both platforms.
# Run this AFTER you've built and pushed images on both x64 and ARM64 machines.

set -e

# Version is sourced from Cargo.toml
SCRIPT_DIR=$(dirname "$(realpath "$0")")
VERSION=$(grep '^version' "$SCRIPT_DIR/../Cargo.toml" | head -1 | cut -d'"' -f2)
MINOR_VERSION=$(echo "$VERSION" | cut -d. -f1,2)
IMAGE_NAME="whiskeyjay/ptn2influx"

echo "Creating multi-arch manifests for ptn2influx..."
echo ""

echo "Creating manifest for :latest"
docker buildx imagetools create \
  -t $IMAGE_NAME:latest \
  $IMAGE_NAME:$VERSION-amd64 \
  $IMAGE_NAME:$VERSION-arm64

echo "Creating manifest for :$VERSION"
docker buildx imagetools create \
  -t $IMAGE_NAME:$VERSION \
  $IMAGE_NAME:$VERSION-amd64 \
  $IMAGE_NAME:$VERSION-arm64

echo "Creating manifest for :$MINOR_VERSION"
docker buildx imagetools create \
  -t $IMAGE_NAME:$MINOR_VERSION \
  $IMAGE_NAME:$VERSION-amd64 \
  $IMAGE_NAME:$VERSION-arm64

echo ""
echo "============================================"
echo "✓ Multi-arch manifests created and pushed!"
echo "============================================"
echo ""
echo "Available tags:"
echo "  - $IMAGE_NAME:latest (amd64 + arm64)"
echo "  - $IMAGE_NAME:$VERSION (amd64 + arm64)"
echo "  - $IMAGE_NAME:$MINOR_VERSION (amd64 + arm64)"
echo ""
