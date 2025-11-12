#!/bin/bash

# This script creates and pushes multi-arch manifests after building on both platforms.
# Run this AFTER you've built and pushed images on both x64 and ARM64 machines.

set -e

# Version configuration
VERSION="0.1.3"
MINOR_VERSION="0.1"
IMAGE_NAME="whiskeyjay/ptn2influx"

echo "Creating multi-arch manifests for ptn2influx..."
echo ""

# Remove old manifests if they exist (they might be cached locally)
docker manifest rm $IMAGE_NAME:latest 2>/dev/null || true
docker manifest rm $IMAGE_NAME:$VERSION 2>/dev/null || true
docker manifest rm $IMAGE_NAME:$MINOR_VERSION 2>/dev/null || true

echo "Creating manifest for :latest"
docker manifest create $IMAGE_NAME:latest \
  --amend $IMAGE_NAME:$VERSION-amd64 \
  --amend $IMAGE_NAME:$VERSION-arm64

echo "Creating manifest for :$VERSION"
docker manifest create $IMAGE_NAME:$VERSION \
  --amend $IMAGE_NAME:$VERSION-amd64 \
  --amend $IMAGE_NAME:$VERSION-arm64

echo "Creating manifest for :$MINOR_VERSION"
docker manifest create $IMAGE_NAME:$MINOR_VERSION \
  --amend $IMAGE_NAME:$VERSION-amd64 \
  --amend $IMAGE_NAME:$VERSION-arm64

echo ""
echo "Pushing manifests..."
docker manifest push $IMAGE_NAME:latest
docker manifest push $IMAGE_NAME:$VERSION
docker manifest push $IMAGE_NAME:$MINOR_VERSION

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
