#!/usr/bin/env bash

set -euo pipefail

no_build=0
open_browser=0

for arg in "$@"; do
    case "$arg" in
        --no-build) no_build=1 ;;
        --open-browser) open_browser=1 ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 1
            ;;
    esac
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
runtime_dir="$repo_root/.runtime"
logs_dir="$runtime_dir/logs"
state_path="$runtime_dir/dev-stack.json"
bootstrap_state_path="$runtime_dir/dev-bootstrap.json"

assert_command_available() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Required command '$1' was not found on PATH." >&2
        exit 1
    fi
}

wait_for_http_endpoint() {
    local name="$1"
    local url="$2"
    local timeout_seconds="${3:-120}"
    local deadline=$((SECONDS + timeout_seconds))

    while (( SECONDS < deadline )); do
        if curl --silent --show-error --fail --max-time 5 "$url" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.75
    done

    echo "$name did not become ready at $url within $timeout_seconds seconds." >&2
    exit 1
}

get_compose_service_container_id() {
    docker compose ps -q "$1" 2>/dev/null | head -n1 | tr -d '\r'
}

get_container_status() {
    local container_id="$1"
    if [[ -z "$container_id" ]]; then
        return 0
    fi

    docker inspect -f "{{.State.Status}}" "$container_id" 2>/dev/null | head -n1 | tr -d '\r' || true
}

get_server_logs() {
    docker compose logs --tail 200 server 2>&1 || true
}

wait_for_api_health() {
    local timeout_seconds="${1:-180}"
    local deadline=$((SECONDS + timeout_seconds))
    local reset_attempted=0

    while (( SECONDS < deadline )); do
        if curl --silent --show-error --fail --max-time 5 "http://127.0.0.1:8080/health" >/dev/null 2>&1; then
            return 0
        fi

        local server_container_id
        server_container_id="$(get_compose_service_container_id "server")"
        local server_status
        server_status="$(get_container_status "$server_container_id")"

        if [[ "$server_status" == "exited" ]]; then
            local logs
            logs="$(get_server_logs)"
            if [[ $reset_attempted -eq 0 && "$logs" == *"migration "* && "$logs" == *"previously applied but has been modified"* ]]; then
                reset_attempted=1
                echo "Detected local migration checksum drift in the dev database. Recreating the local database volume..."
                docker compose down -v
                docker compose up --build -d postgres server
                sleep 2
                continue
            fi

            echo "API container exited before becoming healthy." >&2
            echo >&2
            echo "Server logs:" >&2
            echo "$logs" >&2
            exit 1
        fi

        sleep 0.75
    done

    local logs
    logs="$(get_server_logs)"
    echo "API did not become ready at http://127.0.0.1:8080/health within $timeout_seconds seconds." >&2
    echo >&2
    echo "Server logs:" >&2
    echo "$logs" >&2
    exit 1
}

is_pid_running() {
    local pid="$1"
    [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

json_escape() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    printf '%s' "$value"
}

start_tracked_process() {
    local name="$1"
    local working_directory="$2"
    shift 2
    local stdout_path="$logs_dir/$name.stdout.log"
    local stderr_path="$logs_dir/$name.stderr.log"

    (
        cd "$working_directory"
        nohup "$@" >"$stdout_path" 2>"$stderr_path" &
        printf '%s\n' "$!" >"$runtime_dir/$name.pid"
    )

    local pid
    pid="$(cat "$runtime_dir/$name.pid")"
    rm -f "$runtime_dir/$name.pid"

    printf '{"name":"%s","pid":%s,"stdout":"%s","stderr":"%s"}' \
        "$(json_escape "$name")" \
        "$pid" \
        "$(json_escape "$stdout_path")" \
        "$(json_escape "$stderr_path")"
}

open_browser_url() {
    local url="$1"
    if command -v xdg-open >/dev/null 2>&1; then
        nohup xdg-open "$url" >/dev/null 2>&1 &
    elif command -v open >/dev/null 2>&1; then
        nohup open "$url" >/dev/null 2>&1 &
    fi
}

cleanup() {
    rm -f "$bootstrap_state_path"
}

trap cleanup EXIT

mkdir -p "$runtime_dir" "$logs_dir"

assert_command_available docker
assert_command_available bun
assert_command_available curl

if [[ ! -f "$repo_root/apps/server/.env" ]]; then
    echo "Missing apps/server/.env. Copy apps/server/.env.example before starting the dev stack." >&2
    exit 1
fi

if [[ -f "$state_path" ]]; then
    mapfile -t existing_pids < <(grep -o '"pid":[[:space:]]*[0-9]\+' "$state_path" | grep -o '[0-9]\+' || true)
    for pid in "${existing_pids[@]}"; do
        if is_pid_running "$pid"; then
            echo "Dev stack is already running."
            echo "Stop it first with: bun run dev:stop"
            exit 0
        fi
    done
fi

compose_args=(compose up)
if [[ $no_build -eq 0 ]]; then
    compose_args+=(--build)
fi
compose_args+=(-d postgres server)

echo "Starting PostgreSQL + API..."
docker "${compose_args[@]}"

echo "Waiting for API health..."
wait_for_api_health 180

echo "Starting web client..."
client_process="$(start_tracked_process client "$repo_root" bun --cwd apps/client dev --host 127.0.0.1)"

echo "Waiting for frontend dev server..."
wait_for_http_endpoint "Frontend" "http://127.0.0.1:5173" 180

processes=("$client_process")

if [[ $open_browser -eq 1 ]]; then
    open_browser_url "http://127.0.0.1:5173"
fi

{
    printf '{\n'
    printf '  "startedAt": "%s",\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    printf '  "mode": "web",\n'
    printf '  "processes": [\n'
    for i in "${!processes[@]}"; do
        if [[ "$i" -gt 0 ]]; then
            printf ',\n'
        fi
        printf '    %s' "${processes[$i]}"
    done
    printf '\n  ],\n'
    printf '  "urls": {\n'
    printf '    "api": "http://127.0.0.1:8080",\n'
    printf '    "web": "http://127.0.0.1:5173"\n'
    printf '  }\n'
    printf '}\n'
} >"$state_path"

echo
echo "Dev stack is ready."
echo "API: http://127.0.0.1:8080"
echo "Web: http://127.0.0.1:5173"
echo "Logs:"
for process_json in "${processes[@]}"; do
    name="$(printf '%s' "$process_json" | sed -n 's/.*"name":"\([^"]*\)".*/\1/p')"
    stdout_path="$(printf '%s' "$process_json" | sed -n 's/.*"stdout":"\([^"]*\)".*/\1/p')"
    stderr_path="$(printf '%s' "$process_json" | sed -n 's/.*"stderr":"\([^"]*\)".*/\1/p')"
    echo "  $name: $stdout_path"
    echo "  $name: $stderr_path"
done
echo "Stop everything with: bun run dev:stop"
