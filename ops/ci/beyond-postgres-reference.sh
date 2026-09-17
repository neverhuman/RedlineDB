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

container_id=""

cleanup() {
    status=$?
    if [ -n "$container_id" ]; then
        if [ "${REDLINEDB_POSTGRES_KEEP:-0}" = "1" ]; then
            printf 'keeping Postgres container %s for REDLINEDB_POSTGRES_URL=%s\n' "$container_id" "$REDLINEDB_POSTGRES_URL" >&2
        else
            docker rm -f "$container_id" >/dev/null 2>&1 || true
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

    export REDLINEDB_POSTGRES_PORT="${REDLINEDB_POSTGRES_PORT:-55432}"
    export REDLINEDB_POSTGRES_URL="postgres://redlinedb:postgres@127.0.0.1:${REDLINEDB_POSTGRES_PORT}/redlinedb_beyond"
    postgres_image="${REDLINEDB_POSTGRES_IMAGE:-postgres:16-alpine}"

    container_id="$(
        docker run --rm -d \
            -e POSTGRES_DB=redlinedb_beyond \
            -e POSTGRES_USER=redlinedb \
            -e POSTGRES_PASSWORD=postgres \
            -p "127.0.0.1:${REDLINEDB_POSTGRES_PORT}:5432" \
            --health-cmd "pg_isready -U redlinedb -d redlinedb_beyond" \
            --health-interval 2s \
            --health-timeout 5s \
            --health-retries 30 \
            --health-start-period 2s \
            "$postgres_image"
    )"

    for _ in $(seq 1 60); do
        health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id" 2>/dev/null || true)"
        if [ "$health" = "healthy" ]; then
            break
        fi
        if [ "$health" = "unhealthy" ]; then
            docker logs "$container_id" >&2 || true
            printf 'Postgres container became unhealthy\n' >&2
            exit 1
        fi
        sleep 1
    done

    health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id" 2>/dev/null || true)"
    if [ "$health" != "healthy" ]; then
        docker logs "$container_id" >&2 || true
        printf 'timed out waiting for Postgres container health, last status: %s\n' "$health" >&2
        exit 1
    fi
fi

rtk cargo test -p redlinedb-sql --test beyond_sqlite_manifest --quiet --locked
REDLINEDB_REQUIRE_POSTGRES_REFERENCE=1 rtk cargo test -p redlinedb-sql --test beyond_postgres_reference --quiet --locked
