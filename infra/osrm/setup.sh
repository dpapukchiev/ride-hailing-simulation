#!/usr/bin/env bash
# Download and prepare Berlin OSM data for OSRM.
# Run once before starting the Docker container.
#
# Usage:
#   cd infra/osrm
#   bash setup.sh
#   docker compose up -d

set -euo pipefail

DATA_DIR="$(cd "$(dirname "$0")" && pwd)/data"
PBF_URL="https://download.geofabrik.de/europe/germany/berlin-latest.osm.pbf"
PBF_FILE="$DATA_DIR/berlin-latest.osm.pbf"
OSRM_IMAGE="ghcr.io/project-osrm/osrm-backend:v6.0.0"

mkdir -p "$DATA_DIR"

# Step 1: Download Berlin OSM extract (~120 MB)
if [ ! -f "$PBF_FILE" ]; then
  echo "Downloading Berlin OSM extract..."
  curl -L -o "$PBF_FILE" "$PBF_URL"
else
  echo "Berlin OSM extract already downloaded."
fi

# Step 2: Extract road network
if [ ! -f "$DATA_DIR/berlin-latest.osrm" ]; then
  echo "Extracting road network (osrm-extract)..."
  MSYS_NO_PATHCONV=1 docker run --rm -v "$DATA_DIR:/data" "$OSRM_IMAGE" \
    osrm-extract -p /opt/car.lua /data/berlin-latest.osm.pbf

  echo "Partitioning graph (osrm-partition)..."
  MSYS_NO_PATHCONV=1 docker run --rm -v "$DATA_DIR:/data" "$OSRM_IMAGE" \
    osrm-partition /data/berlin-latest.osrm

  echo "Customizing graph (osrm-customize)..."
  MSYS_NO_PATHCONV=1 docker run --rm -v "$DATA_DIR:/data" "$OSRM_IMAGE" \
    osrm-customize /data/berlin-latest.osrm
else
  echo "OSRM data already prepared."
fi

echo ""
echo "Setup complete. Start the server with:"
echo "  cd infra/osrm && docker compose up -d"
echo ""
echo "Test with:"
echo "  curl 'http://localhost:5000/route/v1/driving/13.388860,52.517037;13.397634,52.529407?overview=full&geometries=geojson'"
