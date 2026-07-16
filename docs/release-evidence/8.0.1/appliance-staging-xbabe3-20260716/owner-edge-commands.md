# Public edge activation — EXACT owner commands (AtomicSoul proxy-caddy-1)

Lane 6 staging surface is UP at **xbabe3:8082** (jain-l6-caddy, canonical routes, Host
`www.neverhuman.org`). The public edge is NOT wired: `proxy-caddy-1` on AtomicSoul is
production and its mutation is owner-gated. Everything below is for the OWNER to run on
AtomicSoul (`ssh atomicsoul`, alias in ~/.ssh/config → 192.168.68.78); nothing here was
executed by Lane 6.

## 0. D5 host line (name the upstream)

```sh
grep -q '192\.168\.68\.54[[:space:]]\+xbabe3' /etc/hosts || \
  echo '192.168.68.54  xbabe3' | sudo tee -a /etc/hosts
```

Note: caddy dials from INSIDE the proxy-caddy-1 container; container name resolution does
not read the host /etc/hosts unless the container shares host network or has an
`--add-host`. The route JSON below therefore dials the IP directly — the /etc/hosts line
is for operator ergonomics (curl checks, ssh).

## 1. Stage the three routes (upstream = Lane 6 staging edge xbabe3:8082)

Canonical routes from `deployment/appliance/caddy-routes.json` (jain-deploy main
d4f7e07da198ff272286d509aeebb8001c0e2ab2) with ONE delta: `dial` rewritten from the
compose-network alias `jain-web-control:8080` (unreachable from AtomicSoul) to
`192.168.68.54:8082`, the staging edge that already terminates /try, /api/public/*, and
/live WS on xbabe3. Save as `/tmp/jain-web-routes.json`:

```json
[
  {"@id":"jain-web-try","match":[{"host":["www.neverhuman.org"],"path":["/try"]}],
   "handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"192.168.68.54:8082"}]}],"terminal":true},
  {"@id":"jain-web-api-public","match":[{"host":["www.neverhuman.org"],"path":["/api/public/*"]}],
   "handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"192.168.68.54:8082"}]}],"terminal":true},
  {"@id":"jain-web-live-slots","match":[{"host":["www.neverhuman.org"],"path_regexp":{"name":"slot","pattern":"^/live/g([0-9]{2})(/.*)?$"}}],
   "handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"192.168.68.54:8082"}]}],"terminal":true}
]
```

## 2. Upsert compare-and-swap into srv0 (per caddy-routes.json `_apply`)

The admin API listens on localhost:2019 inside the container (host curl gets no route —
verified read-only), so drive it with `docker exec`:

```sh
# read current config + Etag
docker exec proxy-caddy-1 wget -qS -O /tmp/cfg.json http://localhost:2019/config/apps/http/servers/srv0/routes 2>&1 | grep -i etag

# for each route id: replace if present at /id/<id>, else append.
for id in jain-web-try jain-web-api-public jain-web-live-slots; do
  route=$(python3 -c "import json;print(json.dumps([r for r in json.load(open('/tmp/jain-web-routes.json')) if r['@id']=='$id'][0]))")
  if docker exec proxy-caddy-1 wget -q -O /dev/null http://localhost:2019/id/$id; then
    echo "$route" | docker exec -i proxy-caddy-1 wget -q -O - --method=PATCH \
      --header 'Content-Type: application/json' --header "If-Match: <Etag-from-read>" \
      --body-file=- http://localhost:2019/id/$id
  else
    echo "$route" | docker exec -i proxy-caddy-1 wget -q -O - --method=POST \
      --header 'Content-Type: application/json' --header "If-Match: <Etag-from-read>" \
      --body-file=- http://localhost:2019/config/apps/http/servers/srv0/routes
  fi
done
```

(If the proxy-caddy-1 image lacks wget/python, run the same verbs with curl from any
container attached to its network namespace: `docker run --rm --network container:proxy-caddy-1 curlimages/curl ...`.)

## 3. Verify from the public side

```sh
curl -s https://www.neverhuman.org/try | head -3                        # cockpit landing HTML
curl -s https://www.neverhuman.org/api/public/capacity                  # {"max_active":10,...}
curl -s -X POST https://www.neverhuman.org/api/public/allocations       # 201 + Secure lease cookie
# browser: follow launch_path /live/gNN/ — the Secure cookie works over the real HTTPS edge.
```

## 4. Later (post-gates): switch the upstream to the canonical shape

When the release is owner-signed and images are pushed via release-atomicsoul.sh
(ATOMICSOUL_PUSH), either run the digest-pinned appliance on AtomicSoul itself (compose
`edge: nht_net` alias `jain-web-control` — routes then revert to the canonical dial
`jain-web-control:8080`, zero delta), or keep the xbabe3 fleet and retarget the dial to
xbabe3's future canonical port. Current xbabe3 port occupancy: 8080 = standalone
`jain-web-control` (another lane's, up since 04:12Z), 8081 = `jain-appliance` compose
project (peer lane), 8082 = Lane 6 staging edge.
