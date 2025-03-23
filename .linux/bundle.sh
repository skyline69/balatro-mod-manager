#!/usr/bin/env bash
set -xe

mkdir -p .linux/bundles
output_dir="$PWD/.linux/bundles/$(date --utc +%Y%m%d_%H%M%SZ)"

# Build image and tag it with a temporary name
docker buildx build -f .linux/Dockerfile . --load -t balatro-mod-manager-temp

# Use the tagged image name
docker run --rm -v "$output_dir:/output" balatro-mod-manager-temp bash -c 'cp /app/bundles/* /output/'
