#!/usr/bin/env sh
set -xe

mkdir -p .linux/bundles
docker run --rm \
  -v "$PWD"/.linux/bundles/"$(date --utc +%Y%m%d_%H%M%SZ)":/output \
  "$(docker buildx build -qf .linux/Dockerfile . --load)" \
  bash -c 'cp /app/bundles/* /output/'
