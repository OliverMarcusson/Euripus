#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
runtime_dir="$repo_root/.runtime"
state_path="$runtime_dir/user-test-stack.json"
bootstrap_state_path="$runtime_dir/user-test-bootstrap.json"

is_pid_running() {
    local pid="$1"
    [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

stop_pid_tree() {
    local pid="$1"
    if ! is_pid_running "$pid"; then
        return 0
    fi

    pkill -TERM -P "$pid" 2>/dev/null || true
    kill -TERM "$pid" 2>/dev/null || true
    sleep 1
    pkill -KILL -P "$pid" 2>/dev/null || true
    kill -KILL "$pid" 2>/dev/null || true
}

extract_pids() {
    local path="$1"
    grep -o '"pid":[[:space:]]*[0-9]\+' "$path" | grep -o '[0-9]\+' || true
}

if [[ -f "$bootstrap_state_path" ]]; then
    bootstrap_pid="$(extract_pids "$bootstrap_state_path" | head -n1)"
    if [[ -n "$bootstrap_pid" ]] && is_pid_running "$bootstrap_pid"; then
        echo "Stopping user-test bootstrap (PID $bootstrap_pid)..."
        stop_pid_tree "$bootstrap_pid"
    fi
    rm -f "$bootstrap_state_path"
fi

if [[ -f "$state_path" ]]; then
    while IFS= read -r pid; do
        if [[ -n "$pid" ]] && is_pid_running "$pid"; then
            echo "Stopping tracked process (PID $pid)..."
            stop_pid_tree "$pid"
        fi
    done < <(extract_pids "$state_path")
    rm -f "$state_path"
fi

if command -v docker >/dev/null 2>&1; then
    echo "Stopping PostgreSQL + API..."
    docker compose down
fi

echo "User-test stack stopped."
