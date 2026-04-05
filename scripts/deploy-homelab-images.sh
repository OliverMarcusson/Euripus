#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
env_file="${EURIPUS_DEPLOY_ENV_FILE:-$repo_root/.env.homelab-images}"

if command -v docker >/dev/null 2>&1; then
  container_cli="docker"
  compose_cmd=(docker compose)
elif command -v podman >/dev/null 2>&1; then
  container_cli="podman"
  compose_cmd=(podman compose)
else
  echo "Neither docker nor podman is available on PATH." >&2
  exit 1
fi

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

printf '%s' "$GHCR_TOKEN" | "$container_cli" login ghcr.io --username "$GHCR_USERNAME" --password-stdin

"${compose_cmd[@]}" "${compose_files[@]}" pull server web
"${compose_cmd[@]}" "${compose_files[@]}" up -d

echo
echo "Homelab deploy complete."
echo "Container CLI: ${container_cli}"
echo "Server image: ${EURIPUS_SERVER_IMAGE}:${EURIPUS_IMAGE_TAG}"
echo "Web image: ${EURIPUS_WEB_IMAGE}:${EURIPUS_IMAGE_TAG}"
if [[ "$EURIPUS_ENABLE_NORDVPN" == "true" ]]; then
  echo "NordVPN override: enabled"
fi
