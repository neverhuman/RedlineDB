# Jain 8.0.1 Caddy persistence, ownership, and CAS audit

- Audited: `2026-07-16T05:50:00Z`–`2026-07-16T05:58:10Z`
- Claim: `codex-mcp-v801-jain-caddy-persistence-audit-20260716T055000Z`
- Mode: read-only. No Caddy/admin API write, reload, signal, container action, route/firewall/file,
  source, checkout, branch/ref, forge, PR, or host mutation occurred.
- Decision: **BLOCKED until both the Jain route declaration and the shared Caddy launch config
  have protected ownership.** The live routes work, and Caddy's autosave already contains them,
  but the current container does not use `--resume` and the host files are not a Git checkout.

## Executive result

The smallest durable correction is not to paste three handlers into the Caddyfile. A direct
Caddyfile adaptation reproduces the request matching, but nests the handlers and drops all three
stable `@id` values. That creates duplicate/reconciliation ambiguity for the existing `/id/<id>`
CAS workflow.

The smallest correction that preserves the exact live JSON is:

1. protected-land the three route objects and their xbabe3 upstream in `jain-deploy`;
2. bind the exact expected Caddy root/routes ETags, current and rollback route hashes, Caddy
   config/autosave hashes, host identity, and shared-compose hash in the signed release spec;
3. make the route change, if any, as one full route-table `PATCH` guarded by `If-Match`; a signed
   promotion must fail on HTTP 412 rather than silently recompute under a new ETag;
4. independently prove Caddy's autosave is byte-semantically equal to live config;
5. protected-land and owner-apply the one-line launch-policy delta that adds `--resume`; and
6. recreate only Caddy, then prove the resumed root config, three IDs, all non-Jain routes, and
   public endpoints are unchanged.

Caddy's own documentation confirms that API changes are autosaved and `caddy run --resume`
restores the last working configuration after restart. It also specifies `Etag` + `If-Match` as
the optimistic-concurrency contract for `/config/` writes:
<https://caddyserver.com/docs/api>.

## Exact live identity

### Shared Caddy process

- Host/container: `atomicsoul` / `proxy-caddy-1`
- Container ID:
  `07d2becdc841a5ac091fbf6c440e49f6231efe0f4c3244d890f68dd8ef01692a`
- Image ID: `sha256:818bec5261db8072f732f941ed335d7f027b1657230c0f62479a4398683e911f`
- Image/config: `caddy:2-alpine`, Caddy v2.11.1
- Current command:
  `caddy run --config /etc/caddy/Caddyfile --adapter caddyfile`
- Restart policy: `unless-stopped`
- Config volume: `ei-arena_caddy_config` mounted at `/config`
- Caddyfile: `/home/ubuntu/NHT/Proxy/Caddyfile` read-only at `/etc/caddy/Caddyfile`
- The shared container and unrelated Neverhuman routes are not Jain-owned and are never a
  direct mutation target of an agent.

### Current config, ETags, and hashes

| Scope | ETag | SHA-256 of returned JSON |
|---|---|---|
| `/config/` | `"/config/ ae3d2633649b120e"` | `58dd8068a1ccdaaee56cdfa4d9a498b9d471d0c34da3aef73fc49e18f7394279` |
| `/config/apps/http/servers/srv0/routes` | `"/config/apps/http/servers/srv0/routes fdab8017ad20d5e4"` | `6c0e90aa2df7c81d4d7f4a4d5997def564b2cb0e2b465c6925859b31a02584ce` |
| `/id/jain-web-try` | `"/config/apps/http/servers/srv0/routes/6 8731719f5bcb6c5c"` | `9253da8b7af73ad342e79a1c7b0da5a7a74652e65baf19bfb07458bda5a43b77` |
| `/id/jain-web-api-public` | `"/config/apps/http/servers/srv0/routes/7 56686f5cbc3eaa84"` | `2e269ecb9ee7ae1e62dbaef07f1ad3d895783bedae28977ac7f0cf66bc918242` |
| `/id/jain-web-live-slots` | `"/config/apps/http/servers/srv0/routes/8 f535b42eed832aba"` | `233f0cd66f342798ac1e62330e86e04512febce4b320cf59dd62bf6783c7c9ed` |

The route array has 13 entries. Jain occupies indexes 6, 7, and 8 immediately before the
`www.neverhuman.org` landing catch-all.

### Exact route map

| ID | Match | Current upstream | Classification |
|---|---|---|---|
| `jain-web-try` | host `www.neverhuman.org`, exact path `/try` | `192.168.68.54:8080` | Final public matcher/ID; interim release binding |
| `jain-web-api-public` | host `www.neverhuman.org`, path `/api/public/*` | `192.168.68.54:8080` | Final public matcher/ID; interim release binding |
| `jain-web-live-slots` | host `www.neverhuman.org`, regexp `^/live/g([0-9]{2})(/.*)?$` | `192.168.68.54:8080` | Final public matcher/ID; guest/epoch implementation incomplete |

None of the three route IDs is legacy. Their public match contracts are the intended final
surface. Their current upstream is an owner-confirmed xbabe3 co-location override, but remains an
**interim/non-release binding** because the backend is a local unsigned/unlabelled image and the
override is not protected or signed.

The existing `www.neverhuman.org -> jain-landing:80` catch-all is the persisted public fallback
and must remain after the three Jain-specific routes. AtomicSoul's old `jain-canary` container is
legacy 7.0.5 and has zero live Caddy references.

Current upstream pool readback reported zero active requests and zero remembered failures for
both `jain-landing:80` and `192.168.68.54:8080` at the observation instant. This is operational
telemetry, not shutdown authority.

## Persistence proof

The config volume already contains `/config/caddy/autosave.json`:

- raw file SHA-256: `dd0ba160450106212ca6523d7a64c96f174a264eaf1f999159d00291e74d7ca2`
- size: 4,593 bytes
- mtime: `2026-07-16T04:17:31.781119795Z`, matching the Jain route application time
- mode/owner: `0600`, root:root
- canonical JSON SHA-256:
  `58dd8068a1ccdaaee56cdfa4d9a498b9d471d0c34da3aef73fc49e18f7394279`
- live root canonical JSON SHA-256:
  `58dd8068a1ccdaaee56cdfa4d9a498b9d471d0c34da3aef73fc49e18f7394279`
- autosave contains all three Jain `@id` values.

Thus the data needed for restart durability is present and exactly matches live config. The sole
runtime gap is launch policy: current Caddy does not pass `--resume`, so it prefers the Caddyfile
on restart and discards the autosaved route set.

### Why a Caddyfile-only patch is not accepted

A read-only `caddy adapt` of the proposed three-handler snippet produced correct matchers and
upstreams, but nested them under the `www.neverhuman.org` subroute and emitted no `@id` fields.
After such a restart:

- `/id/jain-web-try`, `/id/jain-web-api-public`, and `/id/jain-web-live-slots` would be 404;
- the current reconciler would not identify the persisted handlers as its own;
- a re-run could append duplicates; and
- route-level rollback/CAS identity would be lost.

The Caddyfile can remain a cold-start fallback, but the exact autosaved JSON plus `--resume` is the
smallest change preserving the current route identity.

## Protected ownership map

| Surface | Intended owner | Current evidence | Decision |
|---|---|---|---|
| Jain route JSON and CAS/rollback tool | `jain-deploy` | PR #26 branch `claude/appliance-v8.0.1-20260716` at `40e589852eed0bb82544626c4326141be149a6a4`; unmerged | Correct owner, not protected-landed |
| HTTP behavior behind `/try`, `/api/public/*`, `/live/gNN/*` | `jain-web` | protected main `6adcda48e5711dcf5934934236be09fcc959ff10`; current feature branch `13ea4968dde1286111dacf8cd0216fa71e931242`; deployed binary not bound to either | Web owns protocol, not Caddy lifecycle |
| Shared Caddy compose/config volume/Caddyfile | Neverhuman `NHT/Proxy` infrastructure owner | `/home/ubuntu/NHT/Proxy` is **not a Git checkout** | BLOCK: requires protected infra ownership and owner approval |
| Immutable ETag/config/route/rollback binding and two signatures | Jain release authority / `CloudReleaseSpecV1` | current Deploy schema binds only route file path/hash | BLOCK: schema lacks host, ETag, pre/target/rollback hashes |

### Protected Deploy source truth

- `jain-deploy/main`:
  `dd80c5742fd1b1b7544078e1c1f6fef71e5e1a74`
- PR #26 head:
  `40e589852eed0bb82544626c4326141be149a6a4`
- Route file:
  `deployment/appliance/caddy-routes.json`
- Route file SHA-256 at PR #26 head:
  `1421a1ed164bafefccf357bba7b88e1cd067d0436acacc18a9e61182f6cd4f5e`
- Original route-draft commit:
  `8e22d48ec767ae3ad4a016a175008dbe936103d4`

The PR file has exactly the same three IDs, matchers, and terminal behavior, but dials
`jain-web-control:8080`. That alias assumes Caddy and Web share AtomicSoul's `nht_net`. The active
owner-confirmed topology co-locates Web on xbabe3, so the live deployment script rewrote only the
upstream to `192.168.68.54:8080`. This override exists in scratch
`w10/deploy-routes.sh`; it is not protected source.

The current protected `jain-deploy` Caddy implementation is also insufficient for these routes:

- it owns only `/release/canary/<slug>` routes, not the three public route IDs;
- it reads a route array and retries an unguarded full-array `PATCH`;
- it does not send `If-Match` or require an expected ETag; and
- its retry loop can overwrite a concurrent writer.

The scratch W10 script uses `If-Match` for apply, but it automatically refetches/recomputes on
HTTP 412 and its rollback performs three separate unguarded `DELETE /id/<id>` operations. Neither
behavior is acceptable once two owner signatures bind an exact release spec and expected ETag.

### Host config truth

- `/home/ubuntu/NHT/Proxy/Caddyfile` SHA-256:
  `1c1541069702fedd4a633db73abc2f05da2d47acb49b91d8f0a911a936739444`
- `/home/ubuntu/NHT/Proxy/docker-compose.yml` SHA-256:
  `f63fecff87954a99a78fe9877ce47621c179cc4cfaaf5a4b8604312b3df0cd6f`
- neither file is inside a Git checkout;
- Compose does not set a Caddy command and therefore inherits the image default without
  `--resume`.

Jain must not silently claim ownership of this shared Neverhuman stack. The infrastructure owner
must either put the base Compose/Caddyfile in a protected repository or approve a protected,
digest-bound Jain override whose exact base-file hash is a precondition.

## Exact smallest source/config correction

### 1. Route declaration

In the protected Deploy PR, render the selected topology explicitly. For the currently approved
xbabe3 co-location, all three route objects must contain:

```json
"upstreams": [{ "dial": "192.168.68.54:8080" }]
```

Do not leave the topology delta only in a command-line `--upstream` override. The final route file
hash must be bound in `CloudReleaseSpecV1`.

### 2. Signed release fields

Extend `caddy_routes` in `CloudReleaseSpecV1` with at least:

```json
{
  "host": "atomicsoul",
  "caddy_container_image_id": "sha256:818bec5261db8072f732f941ed335d7f027b1657230c0f62479a4398683e911f",
  "expected_root_etag": "\"/config/ ae3d2633649b120e\"",
  "expected_routes_etag": "\"/config/apps/http/servers/srv0/routes fdab8017ad20d5e4\"",
  "pre_config_sha256": "58dd8068a1ccdaaee56cdfa4d9a498b9d471d0c34da3aef73fc49e18f7394279",
  "pre_routes_sha256": "6c0e90aa2df7c81d4d7f4a4d5997def564b2cb0e2b465c6925859b31a02584ce",
  "target_routes_sha256": "<sha256-of-protected-rendered-full-route-array>",
  "rollback_routes_sha256": "<sha256-of-full-route-array-with-only-the-three-jain-ids-removed>",
  "autosave_sha256": "dd0ba160450106212ca6523d7a64c96f174a264eaf1f999159d00291e74d7ca2",
  "base_compose_sha256": "f63fecff87954a99a78fe9877ce47621c179cc4cfaaf5a4b8604312b3df0cd6f",
  "base_caddyfile_sha256": "1c1541069702fedd4a633db73abc2f05da2d47acb49b91d8f0a911a936739444"
}
```

The two owner signatures must cover these exact fields. Placeholder values fail closed.

### 3. Caddy resume override

The minimal reviewed Compose delta is:

```yaml
services:
  caddy:
    command:
      - caddy
      - run
      - --config
      - /etc/caddy/Caddyfile
      - --adapter
      - caddyfile
      - --resume
```

A read-only Compose render against the current base produced exactly that command while preserving
image, restart policy, ports, networks, and mounts. Caddy v2.11.1's local `run --help` confirms
that `--resume` prefers an existing autosave and falls back to `--config` when none exists.

This override must be protected by the shared infra owner. Merely leaving it in a scratch runbook
or invoking `docker compose -f ... -f -` once is not durable operational ownership.

## Exact correction sequence for an authorized owner

This is a design, not an executed script. All `<...>` files must be protected, signed artifacts
from the exact accepted release spec. No credential value belongs in argv, logs, or receipts.

1. Freeze the shared Caddy writer lane and verify the two owner signatures over the exact spec.
2. GET `/config/`, the full `srv0/routes` array, and all three `/id/...` objects in a single
   observation window. Capture bodies, ETags, hashes, Caddy container/image IDs, base file hashes,
   and `/config/caddy/autosave.json`.
3. Require the exact ETags/hashes listed above. Any drift aborts; regenerate the release spec and
   obtain two new signatures. Do not auto-refetch and continue under the old signatures.
4. Render the full target route array from protected source. Remove existing objects only by the
   three exact IDs, insert the three protected objects immediately before the
   `www.neverhuman.org` catch-all, and verify every non-Jain object is byte-semantically identical.
5. If the target canonical hash equals the live routes hash, make no admin write. That is the
   current expected case.
6. Otherwise, perform exactly one full-array request:

```sh
docker exec -i proxy-caddy-1 curl --fail-with-body --silent --show-error \
  --request PATCH \
  --header 'Content-Type: application/json' \
  --header 'If-Match: "/config/apps/http/servers/srv0/routes fdab8017ad20d5e4"' \
  --data-binary @- \
  http://127.0.0.1:2019/config/apps/http/servers/srv0/routes \
  < routes.target.json
```

HTTP 412 is a hard stop. Do not retry within the signed deployment action.

7. GET and hash the route array and all three IDs. Verify public `/`, `/try`,
   `/api/public/capacity`, anonymous `/live/g01/`, Web health, and every shared-route invariant.
8. Require canonical `/config/` JSON to equal canonical autosave JSON. Archive both plus hashes.
9. Require the exact shared base Compose/Caddyfile hashes, render the protected `--resume`
   override, and run `docker compose config` readback. The rendered service must differ only in
   `command`.
10. During an owner-approved edge window, recreate only the Caddy service using the protected
    base and override. Do not stop the Compose project or unrelated services.
11. Read back the new Caddy argv (`--resume` present), container/image IDs, root/routes ETags,
    root/routes hashes, three route IDs/upstreams, autosave equivalence, TLS, all shared vhosts,
    and public Jain endpoints.
12. A process/container/host restart drill is a qualification action. Preserve pre/post config
    receipts and require the same three IDs and non-Jain route projection after recovery.

## Exact rollback design

### Production-route rollback

The safe release rollback removes the three Jain objects atomically while preserving the landing
catch-all and every unrelated route:

1. close Jain admission and preserve Web/guest/SmartCluster state;
2. GET the full route array and current routes-scope ETag;
3. require it matches the rollback action's signed expected ETag;
4. compute one full array by removing only the exact three Jain IDs;
5. compare the non-Jain projection hash with the pre-change value;
6. perform one full-array `PATCH` with `If-Match`; HTTP 412 aborts;
7. prove `/try`, `/api/public/*`, and `/live/gNN/*` fall through to the recorded landing behavior,
   while `/` and every unrelated vhost remain healthy;
8. prove autosave equals the rollback runtime config; and
9. leave `--resume` enabled so the rollback itself is restart-durable.

Do not use three sequential `DELETE /id/<id>` requests: that permits a partial rollback and does
not give one atomic CAS boundary.

### Resume-launch rollback

Only revert `--resume` if the Caddy launch correction itself fails. Restoring the old command and
recreating Caddy loads the old Caddyfile and therefore removes all three Jain routes. To restore
the precise prior ephemeral state, the owner must then CAS-apply the signed pre-change route array
and verify its hashes. This is more fragile than leaving `--resume` enabled, so the preferred
rollback changes route state, not persistence semantics.

## Required tests before merge/apply

- Rust property tests: ID-deduplicating upsert, insert-before-catchall, non-Jain projection
  preservation, atomic three-ID removal, 412 fail-closed, and no-op detection.
- Mutation tests: missing/changed ETag, changed Caddy image/container, duplicate ID, moved/missing
  catch-all, changed upstream, autosave mismatch, base Compose/Caddyfile drift, and placeholder
  signed fields must all fail.
- Integration test against an isolated Caddy container: apply with ETag, verify IDs, restart with
  `--resume`, verify byte-semantic equality, atomically rollback, restart, verify rollback.
- Shared-host rehearsal: render-only first; verify every non-Jain route hash before and after.
- Release binding: route file, rendered full arrays, autosave, host base files, Caddy image,
  ETags, public readback, and rollback receipts all included in the two-signature spec.

## Terminal blockers

1. PR #26 route declaration remains unmerged and its upstream does not match the live topology.
2. Protected Deploy `main` has no production three-route CAS implementation; existing full-array
   writes are unguarded.
3. The shared `NHT/Proxy` directory is not a Git checkout, so its launch policy has no protected
   source or review trail.
4. `CloudReleaseSpecV1` does not bind expected ETags, live/target/rollback route arrays, shared
   host config hashes, or autosave identity.
5. The live Web backend is a local unlabelled image, so the route target is not an immutable
   release artifact.
6. No restart receipt proves the three IDs survive Caddy restart.

**BLOCK.** The live routes are point-in-time functional and their autosave is exact, but the
protected ownership, signed CAS contract, and `--resume` launch policy must land before the routes
are production-durable.
