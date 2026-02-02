#!/usr/bin/env sh
set -eu

IMAGE_NAME="ride-hailing-sim"

docker build -t "$IMAGE_NAME" .
docker run --rm "$IMAGE_NAME"
