# Jain 8.0.1 AtomicSoul + xbabe3 capacity/shutdown audit

- Observation window: `2026-07-16T05:39:21Z`–`2026-07-16T05:46:58Z`
- Claim: `codex-mcp-v801-jain-host-capacity-audit-20260716T053854Z`
- Mode: read-only. No container, process, service, route, firewall, listener, image,
  lease, workload, file, or host state was changed.
- Hosts: `atomicsoul` (`192.168.68.78`) and `xbabe3` (`192.168.68.54`)
- Scope fence: Jain resources only. Neverhuman/Signalhouse and unrelated Jeryu/Veox
  resources were observed only far enough to exclude them and are not shutdown candidates.

## Decision

Exactly **one** running Jain resource is positively safe to stop: AtomicSoul's unrouted legacy
`jain-canary` container. It consumes about `232.8 MiB` and 17 PIDs. Stopping it is optional and
small-capacity only; it is not the critical path.

Everything on the current public request path is `KEEP`:

- AtomicSoul `jain-landing` backs the public site root.
- AtomicSoul's shared Caddy has three live in-memory Jain routes. The shared Caddy container is
  unrelated infrastructure and explicitly excluded from shutdown.
- xbabe3 `jain-web-control` is the live backend for `/try`, `/api/public/*`, and `/live/gNN/*`.
- xbabe3 `scqd.service` is the only live Jain CPU/GPU lease engine. It is idle but required for
  staging and public functionality.

No Jain resource is classified `UNKNOWN` for shutdown. Browser/cockpit session count itself is
`UNKNOWN` because the current Web build exposes no audited session-enumeration interface; that
uncertainty reinforces `KEEP` for the routed Web backend. SmartCluster lease state is exact: zero
queued jobs, zero running jobs, and zero active fabric leases.

## Classification matrix

| Host | Resource | Class | Exact reason |
|---|---|---|---|
| AtomicSoul | `jain-canary` container | **SAFE_TO_STOP** | Legacy 7.0.5 image; zero runtime Caddy references; zero dependent-container references; no host-published port; zero log lines in the prior 6h; not the 7.0.6 rollback target; current Jain routes use xbabe3 instead. Preserve its image and volume. |
| AtomicSoul | `jain-landing` container | **KEEP** | Runtime Caddy config proxies `www.neverhuman.org` root to `jain-landing:80`; 367 log lines in prior 6h. Current public-route backend. |
| AtomicSoul | Jain Caddy route objects | **KEEP** | Three current route IDs point to xbabe3. Shared `proxy-caddy-1` itself is excluded Neverhuman infrastructure and must not be stopped or edited by this lane. |
| xbabe3 | `jain-web-control` container | **KEEP** | Exact live backend of the three public routes; public `/try` returned HTTP 200 during this audit. |
| xbabe3 | `scqd.service` + `jain-smartcluster.slice` | **KEEP** | Required CPU/RTX 3090 lease engine. Idle is expected staging capacity, not supersession. |

## AtomicSoul inventory

### Host capacity

- Uptime at observation: 2 days 13:50; load average `1.04 1.16 1.18`.
- RAM: 29,343,993,856 bytes total; 18,835,718,144 bytes available.
- Swap: 2,147,479,552 bytes total; 4,718,592 bytes used.
- Root/Docker filesystem: 7,936,769,732,608 bytes total;
  1,497,605,705,728 used; 6,039,096,004,608 available (`20%`).
- Host listeners relevant to Jain (`4180`, `8080`, `7443`, `7444`, `7700`): none.
  Jain backends are reached over the Docker network or on xbabe3.
- No `guestd` service and no `g01`–`g10`/slot containers were observed.

### `jain-canary` — SAFE_TO_STOP

- Container ID:
  `780ce7cd8e93772a92c25bd6ef988672e5f7d79ad957e8f56238e6a53f4b9d85`
- Container image ID:
  `sha256:9dcf5cb0a0d6fd6104b593141e529686b853825042db3f1e32933c2182713e06`
- Exact repository digest:
  `image.neverhuman.org/doug/jain_small/jain-sagemaker@sha256:7186a6c57dc8eb5d32aba336f384b5d467add8f2c17e96f8c33cb567a118c830`
- OCI revision/version labels: revision `c2cb8acc36e4e8d8ef6f17c30eb485dbfd86d4f6`,
  version `7.0.5`, artifact-manifest hash `unknown`.
- State: running and healthy since `2026-07-13T15:49:10Z`; restart count 0.
- Process: `/usr/local/bin/jain-web --host 0.0.0.0 --port 4180 ...`; container runs as
  root (`User=0`).
- Restart policy: `unless-stopped`.
- Healthcheck: `/usr/local/bin/jain-web --probe http://127.0.0.1:4180/api/health`.
- Image exposes 4180 and 8080, but container has **zero published host ports**.
- Network: `nht_net`, IP `172.19.0.4/16`.
- Persistent volume: `jain-canary-data` at `/tmp/jain-web`. It must not be removed, pruned,
  or modified by the optional stop.
- Runtime security: writable root; no cap drop; no `no-new-privileges`; no memory/CPU/PID limit.
- Point-in-time usage: 0.00% CPU; 232.8 MiB RAM; 17 PIDs; network I/O 51.5 kB / 642 B;
  block I/O 170 MB / 279 kB.
- Activity: zero Docker log lines in the preceding six hours.
- Dependency proof:
  - live Caddy config contains zero strings referencing `jain-canary`;
  - every other Docker container's inspect document contains zero `jain-canary` references;
  - the only Jain Caddy upstreams are `jain-landing:80` and `192.168.68.54:8080`;
  - release rollback target is 7.0.6, not this image's 7.0.5.

This evidence makes a guarded manual `docker stop` safe. It does **not** authorize `docker rm`,
volume deletion, image deletion, or Docker-wide prune.

### `jain-landing` — KEEP

- Container ID:
  `863b9125bbef39b4a906352256167046631bf5d7a13bf1afc6844b0fe8c6acf4`
- Image ID: `sha256:d0c7807749103be4b1fcd09378f16ed4146c79e76e97ad26d0545004cc67474c`
- Repository digest: `nginx@sha256:f46cb72c7df02710e693e863a983ac42f6a9579058a59a35f1ae36c9958e4ce0`
- State: running since `2026-07-13T15:49:10Z`; restart `unless-stopped`.
- Network: `nht_net`, IP `172.19.0.3/16`; no published host port.
- Read-only bind: `/home/ubuntu/jain-landing` → `/usr/share/nginx/html`.
- Point-in-time usage: 0.00% CPU; 22.22 MiB RAM; 17 PIDs; network I/O
  1.1 MB / 13.3 MB; 367 log lines in preceding six hours.
- Runtime Caddy config proxies the `www.neverhuman.org` root route to `jain-landing:80`.
- It is a current public backend and is not a shutdown candidate.

### Jain route dependencies — KEEP

Shared container `proxy-caddy-1` is Neverhuman infrastructure and is excluded. Read-only access
to its internal admin endpoint showed these live Jain route objects:

| Route ID | Match | Upstream |
|---|---|---|
| `jain-web-try` | host `www.neverhuman.org`, exact path `/try` | `192.168.68.54:8080` |
| `jain-web-api-public` | host `www.neverhuman.org`, path `/api/public/*` | `192.168.68.54:8080` |
| `jain-web-live-slots` | host `www.neverhuman.org`, regexp `^/live/g([0-9]{2})(/.*)?$` | `192.168.68.54:8080` |
| persisted root route | host `www.neverhuman.org` | `jain-landing:80` |

- Runtime Caddy config SHA-256:
  `58dd8068a1ccdaaee56cdfa4d9a498b9d471d0c34da3aef73fc49e18f7394279`.
- Mounted Caddyfile SHA-256:
  `1c1541069702fedd4a633db73abc2f05da2d47acb49b91d8f0a911a936739444`.
- The three dynamic Jain route IDs are present only in live runtime config; the mounted Caddyfile
  contains the landing route but not xbabe3 or `/try`. This is a restart-loss blocker and must be
  resolved by the final CAS-backed release configuration. It is not permission to alter Caddy now.
- A direct AtomicSoul → `192.168.68.54:8080/health` read returned HTTP 200 in 0.000788s.
- A clean external GET of `https://www.neverhuman.org/try` returned HTTP 200 in 0.074s.

## xbabe3 inventory

### Host capacity

- Uptime at observation: 3 days 6:25; load average `0.04 0.11 0.10`.
- RAM: 134,838,870,016 bytes total; 126,216,450,048 bytes available.
- Swap: 8,589,930,496 bytes total; 1,835,008 bytes used.
- Root/Docker/SmartCluster filesystem: 982,240,026,624 bytes total;
  781,713,330,176 used; 190,519,173,120 available (`81%`). Disk pressure, not RAM/GPU,
  is the meaningful capacity concern. This audit does not authorize pruning.
- RTX 3090: UUID `GPU-2857566c-23c5-293a-df02-7c66a7571ac4`, 24,576 MiB total,
  1 MiB used, 0% utilization, 26 C, P8.
- Relevant TCP listeners: only `0.0.0.0:8080` and `[::]:8080` via Docker proxy.
  Nothing listens on 4180, 7443, 7444, or 7700.
- SmartCluster listens only on `/run/jain-smartcluster/scqd.sock`.
- No `guestd` service and no `g01`–`g10`/slot containers were observed.

### `jain-web-control` — KEEP

- Container ID:
  `2fa014b56481504a59fecc7528f13a896a5b71ed31344f31ce25dc6377415b4b`
- Image ID:
  `sha256:388e20bc5ea98ab7f689739c2f8fefbcb25a9b972b388b28635cb2a33d68b069`
- Local name/digest:
  `image.local/jain/web-control@sha256:388e20bc5ea98ab7f689739c2f8fefbcb25a9b972b388b28635cb2a33d68b069`.
  This is a local Docker identity, not a proven public registry manifest digest.
- Labels: none; no source/revision/provenance label is bound.
- State: running and healthy since `2026-07-16T04:20:26Z`.
- Process/user: `/jain-web-control --host 0.0.0.0 --port 8080`, `65534:65534`.
- Healthcheck: `/jain-web-control --probe http://127.0.0.1:8080/health`.
- Restart policy: `unless-stopped`.
- Port: 8080 published on all IPv4 and IPv6 interfaces.
- Runtime security gaps: writable root; no cap drop; no `no-new-privileges`; no resource limits;
  no provenance labels. These are deployment blockers, not shutdown authority.
- Point-in-time use: 0.00% CPU; 648 KiB RAM; one PID; 54.3 kB / 145 kB network I/O.
- Point-in-time established TCP connections on port 8080: zero. That does not make the backend
  disposable because all three public routes target it.
- Internal and AtomicSoul health probes returned HTTP 200. External `/try` returned HTTP 200.

### `scqd.service` — KEEP

- Unit: enabled, active/running since `2026-07-16T01:04:56Z`.
- Main PID: `3110219`; user root; launch path `/usr/local/bin/scqd-launch.sh`.
- Service policy: `Restart=on-failure`, `RestartSec=2s`, no watchdog; delegated controllers in
  `jain-smartcluster.slice`; `NoNewPrivileges=no`; `ProtectSystem=full`;
  `ProtectHome=read-only`; `TasksMax=4096`.
- Current/peak service memory: 27,611,136 / 60,387,328 bytes; 131 tasks;
  CPU usage 4,708,634,000 ns at observation.
- Versions: `scqd 8.0.0`, protocol 1.3, mode Full; `scq 8.0.0`.
- Binary/config hashes:
  - `/usr/local/bin/scqd`: `0cb26b6f13a6ea2581df65ef64e7ab1082af5cbf33173311227fc117e07c428e`
  - `/usr/local/bin/scq`: `445ba9bf60063dced11bc145af0b21dc6beda134d3b101ffc677f62aa1f043db`
  - `/usr/local/bin/scqd-launch.sh`: `7f7ac70e799460ec2121a3e2b35d6b4a2e9bd24f066c387ed557b5ab599dfd7f`
  - `/usr/libexec/jain/jain-worker`: `e5271c6d59aff411ac5bc2d1a4f8dd3671e936de27a6429088e988042179de41`
  - `/etc/jain-smartcluster/scqd.toml`: `02c38ea03a5f005e5ba6d998f187567b7f2878df18a04558a2c032b2e164b388`
- State: `/var/lib/jain-smartcluster/scqd.redb`; artifact root
  `/var/lib/jain-smartcluster/artifacts`; exclusive GPU policy; worker UID/GID 996/987.
- Lease readback:
  - doctor: `queued=0 running=0`;
  - `scq --json ps`: empty jobs array;
  - fabric status: disabled local executor, zero nodes, zero commitments, zero queued jobs,
    **zero active leases**;
  - two retained jobs exist in history and are both terminal `completed`; neither has an active
    attempt.
- Capacity: CPU 128,000 millicores; memory 134,838,870,016 bytes; scratch
  195,145,158,656 bytes; GPU healthy with 23,192,823,398 / 25,769,803,776 bytes reported free.

`scqd` is idle by design and is the only observed lease engine. Do not stop it to make room for
the final staging engine; the correct owner flow is an explicit drain/replace/recovery test once
the accepted 8.0.1 service exists.

## Explicit exclusions and false positives

- `proxy-caddy-1` is shared Neverhuman infrastructure. Only its Jain route readback was used.
- Veox synthetic-CI containers on AtomicSoul are unrelated and excluded even when their image
  names contain historical Jain strings.
- xbabe3's Jeryu/GitLab stack is unrelated. A process-table match on user text `jain-worker`
  resolved to a PostgreSQL process inside the Jeryu GitLab container; it is not a Jain worker and
  is excluded.
- No action is authorized against Docker images, volumes, build cache, Caddy, firewall, Jeryu,
  Signalhouse, Neverhuman, or any host-wide prune target.

## Exact guarded owner stop/readback sequence

This sequence is for a later authorized owner. It stops only `jain-canary`; it never removes the
container, its volume, or its image. It deliberately fails if route/dependency/image facts drift.
It was **not run** during this audit.

```bash
set -euo pipefail
: "${APPLY_SAFE_JAIN_STOP:?set APPLY_SAFE_JAIN_STOP=1 only after owner authorization}"
test "$APPLY_SAFE_JAIN_STOP" = 1

ssh -o BatchMode=yes atomicsoul 'bash -se' <<'REMOTE'
set -euo pipefail

name=jain-canary
expected_id=780ce7cd8e93772a92c25bd6ef988672e5f7d79ad957e8f56238e6a53f4b9d85
expected_image=sha256:9dcf5cb0a0d6fd6104b593141e529686b853825042db3f1e32933c2182713e06

test "$(docker inspect --format '{{.Id}}' "$name")" = "$expected_id"
test "$(docker inspect --format '{{.Image}}' "$name")" = "$expected_image"
test "$(docker inspect --format '{{.State.Status}}' "$name")" = running
test "$(docker inspect "$name" | jq '[.[0].NetworkSettings.Ports[]? | select(. != null)] | length')" = 0

runtime_config="$(docker exec proxy-caddy-1 wget -qO- http://127.0.0.1:2019/config/)"
test "$(jq '[.. | strings | select(contains("jain-canary"))] | length' <<<"$runtime_config")" = 0
test "$(jq '[.. | strings | select(contains("jain-landing:80"))] | length' <<<"$runtime_config")" -ge 1
test "$(jq '[.. | strings | select(contains("192.168.68.54:8080"))] | length' <<<"$runtime_config")" = 3

all_ids="$(docker ps -aq)"
test "$(docker inspect $all_ids | jq --arg id "$expected_id" \
  '[.[] | select(.Id != $id) | select(tostring | contains("jain-canary"))] | length')" = 0

# Fail closed if the formerly quiet container has received application traffic recently.
test "$(docker logs --since 15m "$name" 2>&1 | wc -l)" = 0

# Reprove both current public dependencies before the only mutation.
curl --fail --silent --show-error --max-time 10 https://www.neverhuman.org/ -o /dev/null
curl --fail --silent --show-error --max-time 10 https://www.neverhuman.org/try -o /dev/null
curl --fail --silent --show-error --max-time 5 http://192.168.68.54:8080/health -o /dev/null

docker stop --time 30 "$name"

test "$(docker inspect --format '{{.State.Status}}' "$name")" = exited
test "$(docker ps --quiet --filter name="^/${name}$")" = ""

# Jain's actual route backends must remain healthy and unchanged.
curl --fail --silent --show-error --max-time 10 https://www.neverhuman.org/ -o /dev/null
curl --fail --silent --show-error --max-time 10 https://www.neverhuman.org/try -o /dev/null
curl --fail --silent --show-error --max-time 5 http://192.168.68.54:8080/health -o /dev/null
docker exec proxy-caddy-1 wget -qO- http://127.0.0.1:2019/config/ \
  | sha256sum
docker inspect --format \
  'name={{.Name}} state={{.State.Status}} image={{.Image}} restart={{.HostConfig.RestartPolicy.Name}}' \
  "$name"
docker volume inspect jain-canary-data >/dev/null
REMOTE
```

Expected freed live capacity is approximately 233 MiB RAM and 17 PIDs. If any precondition fails,
do not stop anything; regenerate this audit. If an unexpected dependency regression follows the
stop, preserve all state and use only `docker start jain-canary` while the owner investigates—do
not remove or rebuild it.

## Terminal acceptance

- `SAFE_TO_STOP`: one resource (`jain-canary`), guarded and optional.
- `KEEP`: `jain-landing`, three Jain Caddy route objects, `jain-web-control`, and `scqd.service`.
- `UNKNOWN`: no shutdown resource; current Web browser-session enumeration remains unavailable.
- Capacity conclusion: stopping the legacy canary yields only ~233 MiB. xbabe3 already has ample
  RAM/GPU capacity; its 81%-used root filesystem and missing final signed images/services/routes
  are the real staging blockers. No host cleanup or shutdown was performed.
