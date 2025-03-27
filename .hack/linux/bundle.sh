#!/usr/bin/env bash
set -xe

WORKDIR="$PWD/.hack/linux"
OUTDIR="${WORKDIR}/bundles/$(date --utc +%Y%m%d_%H%M%SZ)"
mkdir -p "${OUTDIR}"

# Build image and tag it with a temporary name
docker buildx build -f "${WORKDIR}"/Dockerfile . --load -t balatro-mod-manager:temp

# Use the tagged image name
docker run --rm -v "${OUTDIR}:/output" balatro-mod-manager:temp bash -c 'cp /app/bundles/* /output/'
