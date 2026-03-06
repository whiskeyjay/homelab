#!/bin/bash

# This script creates and pushes multi-arch manifests after building on both platforms.
# Run this AFTER you've built and pushed images on both x64 and ARM64 machines.

set -e

# Version configuration
VERSION="0.1.1"
MINOR_VERSION="0.1"
IMAGE_NAME="whiskeyjay/doh-proxy"

echo "Creating multi-arch manifests for doh-proxy..."
echo ""

echo "Creating and pushing manifest for :latest"
docker buildx imagetools create -t $IMAGE_NAME:latest \
  $IMAGE_NAME:$VERSION-amd64 \
  $IMAGE_NAME:$VERSION-arm64

echo "Creating and pushing manifest for :$VERSION"
docker buildx imagetools create -t $IMAGE_NAME:$VERSION \
  $IMAGE_NAME:$VERSION-amd64 \
  $IMAGE_NAME:$VERSION-arm64

echo "Creating and pushing manifest for :$MINOR_VERSION"
docker buildx imagetools create -t $IMAGE_NAME:$MINOR_VERSION \
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
echo "  - $IMAGE_NAME:$VERSION-amd64"
echo "  - $IMAGE_NAME:$VERSION-arm64"
echo ""
