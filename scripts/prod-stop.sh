#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
env_file="${EURIPUS_DEPLOY_ENV_FILE:-$repo_root/.env.homelab-images}"

if command -v docker >/dev/null 2>&1; then
  compose_cmd=(docker compose)
elif command -v podman >/dev/null 2>&1; then
  compose_cmd=(podman compose)
else
  echo "Neither docker nor podman is available on PATH." >&2
  exit 1
fi

if [[ -f "$env_file" ]]; then
  # shellcheck disable=SC1090
  source "$env_file"
fi

: "${EURIPUS_ENABLE_NORDVPN:=false}"

compose_files=(
  "-f" "docker-compose.homelab.yml"
)

if [[ "$EURIPUS_ENABLE_NORDVPN" == "true" ]]; then
  compose_files+=("-f" "docker-compose.homelab.nordvpn.yml")
fi

cd "$repo_root"

echo "==> Stopping Euripus production stack"
"${compose_cmd[@]}" "${compose_files[@]}" down

echo
echo "Euripus production stack stopped."
