#!/usr/bin/env bash
# Postgres-backed beyond-SQLite reference lane.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

if ! command -v rtk >/dev/null 2>&1; then
    rtk() {
        "$@"
    }
fi

compose_file="$repo_root/ops/ci/beyond-postgres.compose.yml"
compose_project="${REDLINEDB_POSTGRES_COMPOSE_PROJECT:-redlinedb-beyond-postgres}"
compose_started=0

cleanup() {
    status=$?
    if [ "$compose_started" -eq 1 ]; then
        if [ "${REDLINEDB_POSTGRES_KEEP:-0}" = "1" ]; then
            printf 'keeping Postgres compose service for REDLINEDB_POSTGRES_URL=%s\n' "$REDLINEDB_POSTGRES_URL" >&2
        else
            docker compose -p "$compose_project" -f "$compose_file" down --volumes --remove-orphans >&2
        fi
    fi
    exit "$status"
}
trap cleanup EXIT

if [ -z "${REDLINEDB_POSTGRES_URL:-}" ]; then
    if ! command -v docker >/dev/null 2>&1; then
        printf 'docker is required when REDLINEDB_POSTGRES_URL is unset; install Docker or export REDLINEDB_POSTGRES_URL=postgres://redlinedb:postgres@host:port/redlinedb_beyond\n' >&2
        exit 2
    fi
    if ! docker compose version >/dev/null 2>&1; then
        printf 'docker compose is required when REDLINEDB_POSTGRES_URL is unset; install Compose v2 or export REDLINEDB_POSTGRES_URL=postgres://redlinedb:postgres@host:port/redlinedb_beyond\n' >&2
        exit 2
    fi

    export REDLINEDB_POSTGRES_PORT="${REDLINEDB_POSTGRES_PORT:-55432}"
    export REDLINEDB_POSTGRES_URL="postgres://redlinedb:postgres@127.0.0.1:${REDLINEDB_POSTGRES_PORT}/redlinedb_beyond"

    docker compose -p "$compose_project" -f "$compose_file" up -d >&2
    compose_started=1
    container_id="$(docker compose -p "$compose_project" -f "$compose_file" ps -q postgres)"
    if [ -z "$container_id" ]; then
        printf 'Postgres compose service did not create a container\n' >&2
        exit 1
    fi

    for _ in $(seq 1 60); do
        health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id" 2>/dev/null || true)"
        if [ "$health" = "healthy" ]; then
            break
        fi
        if [ "$health" = "unhealthy" ]; then
            docker compose -p "$compose_project" -f "$compose_file" logs postgres >&2 || true
            printf 'Postgres compose service became unhealthy\n' >&2
            exit 1
        fi
        sleep 1
    done

    health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id" 2>/dev/null || true)"
    if [ "$health" != "healthy" ]; then
        docker compose -p "$compose_project" -f "$compose_file" logs postgres >&2 || true
        printf 'timed out waiting for Postgres compose health, last status: %s\n' "$health" >&2
        exit 1
    fi
fi

rtk cargo test -p redlinedb-sql --test beyond_sqlite_manifest --quiet --locked
rtk cargo test -p redlinedb-sql --test beyond_postgres_reference --quiet --locked
