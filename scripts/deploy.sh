#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
env_file="${EURIPUS_DEPLOY_ENV_FILE:-$repo_root/.env.selfhosted-images}"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Required command '$1' was not found on PATH." >&2
    exit 1
  fi
}

warn() {
  echo "Warning: $*" >&2
}

info() {
  echo "==> $*"
}

default_compose_project_name() {
  basename "$repo_root" | tr '[:upper:]' '[:lower:]'
}

get_compose_service_container_id() {
  local service_name="$1"
  local project_name="${COMPOSE_PROJECT_NAME:-$(default_compose_project_name)}"
  local container_id=""

  container_id="$(
    "$container_cli" ps -aq \
      --filter "label=com.docker.compose.project=$project_name" \
      --filter "label=com.docker.compose.service=$service_name" \
      --format "{{.ID}}" 2>/dev/null | head -n1 | tr -d '\r'
  )"
  if [[ -n "$container_id" ]]; then
    printf '%s\n' "$container_id"
    return 0
  fi

  container_id="$(
    "$container_cli" ps -aq \
      --filter "label=io.podman.compose.project=$project_name" \
      --filter "label=io.podman.compose.service=$service_name" \
      --format "{{.ID}}" 2>/dev/null | head -n1 | tr -d '\r'
  )"
  if [[ -n "$container_id" ]]; then
    printf '%s\n' "$container_id"
    return 0
  fi

  container_id="$(
    "$container_cli" ps -aq \
      --filter "name=${project_name}_${service_name}_" \
      --format "{{.ID}}" 2>/dev/null | head -n1 | tr -d '\r'
  )"
  if [[ -n "$container_id" ]]; then
    printf '%s\n' "$container_id"
    return 0
  fi

  return 1
}

get_container_health() {
  local container_id="$1"
  if [[ -z "$container_id" ]]; then
    return 0
  fi

  "$container_cli" inspect -f "{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}" "$container_id" 2>/dev/null | head -n1 | tr -d '\r' || true
}

wait_for_service_health() {
  local service_name="$1"
  local timeout_seconds="${2:-180}"
  local deadline=$((SECONDS + timeout_seconds))

  while (( SECONDS < deadline )); do
    local container_id
    if ! container_id="$(get_compose_service_container_id "$service_name")"; then
      warn "failed to resolve container id for service '$service_name'; retrying"
      sleep 1
      continue
    fi

    local health
    if ! health="$(get_container_health "$container_id")"; then
      warn "failed to inspect health for service '$service_name'; retrying"
      sleep 1
      continue
    fi

    health="${health//$'\n'/}"
    health="${health//$'\r'/}"

    if [[ "$health" == *"healthy"* || "$health" == *"running"* ]]; then
      return 0
    fi

    sleep 1
  done

  echo "Service '$service_name' did not become healthy within $timeout_seconds seconds." >&2
  return 1
}

run_psql_scalar() {
  local sql="$1"
  printf '%s\n' "$sql" | "${compose_cmd[@]}" "${compose_files[@]}" exec -T postgres sh -lc \
    'psql -v ON_ERROR_STOP=1 -qtAX -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
}

repair_sqlx_migration_checksums() {
  local migrations_table_exists
  if ! migrations_table_exists="$(run_psql_scalar "SELECT to_regclass('_sqlx_migrations') IS NOT NULL;")"; then
    warn "failed to inspect _sqlx_migrations; continuing without checksum repair"
    return 0
  fi
  migrations_table_exists="${migrations_table_exists//$'\n'/}"
  migrations_table_exists="${migrations_table_exists//$'\r'/}"

  if [[ "$migrations_table_exists" != "t" ]]; then
    return 0
  fi

  local server_image_ref="${EURIPUS_SERVER_IMAGE}:${EURIPUS_IMAGE_TAG}"
  local temp_dir
  temp_dir="$(mktemp -d)"
  local temp_container_id=""
  local repaired_versions=()

  if ! temp_container_id=$("$container_cli" create "$server_image_ref"); then
    warn "failed to create temporary container for $server_image_ref; continuing without checksum repair"
    rm -rf "$temp_dir"
    return 0
  fi

  if ! "$container_cli" cp "$temp_container_id:/app/migrations/." "$temp_dir/"; then
    warn "failed to copy migrations from $server_image_ref; continuing without checksum repair"
    "$container_cli" rm -f "$temp_container_id" >/dev/null 2>&1 || true
    rm -rf "$temp_dir"
    return 0
  fi

  shopt -s nullglob
  for migration_path in "$temp_dir"/*.sql; do
    local migration_name
    migration_name="$(basename "$migration_path")"
    local version="${migration_name%%_*}"
    if [[ ! "$version" =~ ^[0-9]+$ ]]; then
      continue
    fi

    local checksum_hex
    checksum_hex="$(sha384sum "$migration_path" | awk '{print $1}')"
    local update_result
    if ! update_result="$(run_psql_scalar "WITH updated AS (UPDATE _sqlx_migrations SET checksum = decode('$checksum_hex', 'hex') WHERE version = $version AND success = TRUE AND checksum <> decode('$checksum_hex', 'hex') RETURNING version) SELECT COALESCE(string_agg(version::text, ','), '') FROM updated;")"; then
      warn "failed to repair checksum for migration version $version; continuing"
      continue
    fi
    update_result="${update_result//$'\n'/}"
    update_result="${update_result//$'\r'/}"

    if [[ -n "$update_result" ]]; then
      repaired_versions+=("$version")
    fi
  done
  shopt -u nullglob

  if (( ${#repaired_versions[@]} > 0 )); then
    printf 'Repaired sqlx migration checksum drift for version(s): %s\n' "${repaired_versions[*]}"
  fi

  "$container_cli" rm -f "$temp_container_id" >/dev/null 2>&1 || true
  rm -rf "$temp_dir"
}

get_server_logs() {
  "${compose_cmd[@]}" "${compose_files[@]}" logs --tail 200 server 2>&1 || true
}

wait_for_server_health() {
  local timeout_seconds="${1:-180}"
  local deadline=$((SECONDS + timeout_seconds))

  while (( SECONDS < deadline )); do
    local server_container_id
    if ! server_container_id="$(get_compose_service_container_id server)"; then
      warn "failed to resolve server container id; retrying"
      sleep 1
      continue
    fi

    local server_status
    if ! server_status="$(get_container_health "$server_container_id")"; then
      warn "failed to inspect server health; retrying"
      sleep 1
      continue
    fi

    server_status="${server_status//$'\n'/}"
    server_status="${server_status//$'\r'/}"

    if [[ "$server_status" == *"healthy"* ]]; then
      return 0
    fi

    if [[ "$server_status" == *"exited"* || "$server_status" == *"unhealthy"* ]]; then
      local logs
      logs="$(get_server_logs)"
      echo "Server failed to become healthy during deployment." >&2
      echo >&2
      echo "Server logs:" >&2
      echo "$logs" >&2
      exit 1
    fi

    sleep 1
  done

  local logs
  logs="$(get_server_logs)"
  echo "Server did not become healthy within $timeout_seconds seconds." >&2
  echo >&2
  echo "Server logs:" >&2
  echo "$logs" >&2
  return 1
}

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
: "${EURIPUS_IMAGE_TAG:=selfhosted-latest}"
: "${EURIPUS_ENABLE_NORDVPN:=false}"
: "${GHCR_USERNAME:?Set GHCR_USERNAME in the environment or $env_file before deploying.}"
: "${GHCR_TOKEN:?Set GHCR_TOKEN in the environment or $env_file before deploying.}"

export EURIPUS_SERVER_IMAGE EURIPUS_WEB_IMAGE EURIPUS_IMAGE_TAG EURIPUS_ENABLE_NORDVPN

compose_files=(
  "-f" "docker-compose.selfhosted.yml"
)

if [[ "$EURIPUS_ENABLE_NORDVPN" == "true" ]]; then
  compose_files+=("-f" "docker-compose.selfhosted.nordvpn.yml")
fi

cd "$repo_root"

require_command sha384sum

info "Logging in to ghcr.io"
printf '%s' "$GHCR_TOKEN" | "$container_cli" login ghcr.io --username "$GHCR_USERNAME" --password-stdin

server_image_ref="${EURIPUS_SERVER_IMAGE}:${EURIPUS_IMAGE_TAG}"

info "Stopping existing Euripus stack"
"${compose_cmd[@]}" "${compose_files[@]}" down
info "Pulling Euripus images"
"${compose_cmd[@]}" "${compose_files[@]}" pull postgres meilisearch server web
info "Starting PostgreSQL"
"${compose_cmd[@]}" "${compose_files[@]}" up -d postgres
info "Waiting for PostgreSQL health"
wait_for_service_health postgres 180 || exit 1
info "Repairing SQLx migration checksums if needed"
repair_sqlx_migration_checksums
info "Starting remaining Euripus services"
"${compose_cmd[@]}" "${compose_files[@]}" up -d meilisearch server web
info "Waiting for server health"
wait_for_server_health 180 || exit 1

echo
echo "Euripus deploy complete."
echo "Container CLI: ${container_cli}"
echo "Server image: ${server_image_ref}"
echo "Web image: ${EURIPUS_WEB_IMAGE}:${EURIPUS_IMAGE_TAG}"
if [[ "$EURIPUS_ENABLE_NORDVPN" == "true" ]]; then
  echo "NordVPN override: enabled"
fi
