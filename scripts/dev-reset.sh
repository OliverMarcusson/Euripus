#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dev_stop_script="$script_dir/dev-stop.sh"
dev_start_script="$script_dir/dev-start.sh"

if ! command -v docker >/dev/null 2>&1; then
    echo "Required command 'docker' was not found on PATH." >&2
    exit 1
fi

echo "Stopping existing dev stack..."
bash "$dev_stop_script"

echo "Removing local PostgreSQL and Meilisearch volumes..."
docker compose down -v

echo "Rebuilding and restarting dev stack..."
bash "$dev_start_script"
