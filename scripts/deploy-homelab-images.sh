#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
env_file="${EURIPUS_DEPLOY_ENV_FILE:-$repo_root/.env.homelab-images}"

if [[ -f "$env_file" ]]; then
  # shellcheck disable=SC1090
  source "$env_file"
fi

: "${EURIPUS_SERVER_IMAGE:=ghcr.io/olivermarcusson/euripus-server}"
: "${EURIPUS_WEB_IMAGE:=ghcr.io/olivermarcusson/euripus-web}"
: "${EURIPUS_IMAGE_TAG:=homelab-latest}"
: "${EURIPUS_ENABLE_NORDVPN:=false}"
: "${GHCR_USERNAME:?Set GHCR_USERNAME in the environment or $env_file before deploying.}"
: "${GHCR_TOKEN:?Set GHCR_TOKEN in the environment or $env_file before deploying.}"

export EURIPUS_SERVER_IMAGE EURIPUS_WEB_IMAGE EURIPUS_IMAGE_TAG EURIPUS_ENABLE_NORDVPN

compose_files=(
  "-f" "docker-compose.homelab.yml"
)

if [[ "$EURIPUS_ENABLE_NORDVPN" == "true" ]]; then
  compose_files+=("-f" "docker-compose.homelab.nordvpn.yml")
fi

cd "$repo_root"

printf '%s' "$GHCR_TOKEN" | docker login ghcr.io --username "$GHCR_USERNAME" --password-stdin

docker compose "${compose_files[@]}" pull server web
docker compose "${compose_files[@]}" up -d

echo
echo "Homelab deploy complete."
echo "Server image: ${EURIPUS_SERVER_IMAGE}:${EURIPUS_IMAGE_TAG}"
echo "Web image: ${EURIPUS_WEB_IMAGE}:${EURIPUS_IMAGE_TAG}"
if [[ "$EURIPUS_ENABLE_NORDVPN" == "true" ]]; then
  echo "NordVPN override: enabled"
fi
