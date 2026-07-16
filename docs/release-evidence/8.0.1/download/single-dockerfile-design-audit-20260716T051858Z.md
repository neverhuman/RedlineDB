# Jain 8.0.1 single-Dockerfile publication audit

- Generated: `2026-07-16T05:18:58Z`
- Claim: `codex-mcp-v801-single-dockerfile-audit-20260716T0510Z`
- Scope: read-only design and registry/source evidence audit. No Deploy source, PR, CI,
  registry, route, host, or worktree was mutated.
- Decision: **BLOCKED — do not advertise the current Dockerfile as the final 8.0.1
  single-file download.**

## Executive result

The smallest honest zero-local-context download is a one-instruction Dockerfile whose only
instruction is a digest-pinned `FROM` for an already built, tested, signed, non-root Jain 8.0.1
standalone image. The downloaded file must not compile Jain, copy local files, install packages,
or accept a caller-selected image argument.

No image available at the time of this audit can safely fill that digest:

1. The remotely available and signed `image.neverhuman.org/veox/jain:8.0.1-gpu` image is
   addressable by OCI index digest
   `sha256:ffbbaf48bda7253785b2d32436eb0cbf1d0ff927a41593b907f7307e07075871`, but its OCI
   config is root (`User="0"`), has no healthcheck, and exposes both `8080/tcp` and `8888/tcp`.
   It therefore fails the required runtime contract and **must not** be used as the final base.
2. Four newer role images exist only in the local Docker image store. Registry readback by their
   apparent digests returned `NOT FOUND`; none is a published customer artifact and none is a
   single standalone appliance.
3. Deploy PR #26 is a cloud multi-role topology (Web control plus `scqd`, with workers launched
   as jobs). A Dockerfile alone cannot encode its multi-container lifecycle, GPU reservations,
   guest/WAL authority, private gateway, or Caddy routing. The cloud deployment and the customer
   single-container download must be separately named and separately qualified.

## Exact final Dockerfile contract

The final published file is exactly one line plus its terminating newline:

```dockerfile
FROM image.neverhuman.org/veox/jain@sha256:<APPROVED_8_0_1_STANDALONE_OCI_INDEX_SHA256>
```

`<APPROVED_8_0_1_STANDALONE_OCI_INDEX_SHA256>` is a render token, not publishable text. The
renderer must replace it with exactly 64 lowercase hexadecimal characters obtained by registry
readback after publishing the accepted image. It must fail if the token remains, if the digest is
not bound in `CloudReleaseSpecV1`, or if the registry's digest differs.

This is intentionally not filled with the currently signed `ffbbaf48...` digest because doing so
would bless the root/no-health/two-port image. The final image must inherit all of the following;
the wrapper Dockerfile must not try to repair an unsafe image by overriding only `USER`,
`HEALTHCHECK`, `ENTRYPOINT`, or `EXPOSE`:

- non-root numeric user that has been exercised in the actual runtime tests;
- one public application port, `8080/tcp`;
- a bounded in-image healthcheck for the real readiness endpoint;
- immutable Jain 8.0.1 source/tag/tree, SBOM, provenance, and revision labels;
- a fixed entrypoint and command accepting no arbitrary shell/argv execution;
- successful read-only-root, tmpfs-state, dropped-capabilities, and
  `no-new-privileges` execution;
- for the GPU artifact, successful NVIDIA allocation and fail-closed behavior when a
  GPU-required request cannot obtain a GPU.

The digest reference is the authority. A mutable tag may exist for human discovery, but it must
not appear as an unpinned `FROM` and must not be the verification authority.

## Current public bytes and why they fail

At `2026-07-16T04:18:17Z`, the independently captured acceptance audit recorded:

| Object | Observed result |
|---|---|
| URL | `https://downloads.neverhuman.org/jain/Dockerfile.sagemaker.gpu` |
| HTTP | `200` |
| Last-Modified | `Thu, 16 Jul 2026 03:57:54 GMT` |
| Size | `10,424` bytes |
| SHA-256 | `8863305584dfa154feb319ece024aaa05a962d866f739cca27f5550f9d109f36` |
| Local-context dependency | 16 `COPY` instructions across apps, crates, scripts, vendor, and artifacts |
| Container user | final `USER 0` |
| Healthcheck | absent |
| Exposed ports | `8080` and `4180` |
| SmartCluster reference | loopback URL and stale `jain-smartcluster-v8.0.0-split.0` |
| File signature | absent; only the checksum list is published |

The checksum bundle is internally consistent and the public key is reachable. Its relevant
hashes are:

- `sha256sums.txt` entry for the Dockerfile:
  `8863305584dfa154feb319ece024aaa05a962d866f739cca27f5550f9d109f36`
- `cosign.pub`:
  `2385a8bc4a19af419973768a346eaf9452bdc2d5602d58e648232be974a696cf`

That proves transport consistency, not release acceptance. The authoritative detailed capture is
`public-download-acceptance-audit-20260716.json` in this directory; all DF-01 through DF-09
acceptance rows remain incomplete there.

## Current image evidence

### Remote signed image (reachable, but unusable for the final wrapper)

`docker buildx imagetools inspect image.neverhuman.org/veox/jain:8.0.1-gpu` resolved:

- OCI index: `sha256:ffbbaf48bda7253785b2d32436eb0cbf1d0ff927a41593b907f7307e07075871`
- linux/amd64 manifest:
  `sha256:15466f370964d5f08067581c3053567df4d17eb4b39d9928809a640a402b484f`
- OCI config:
  `sha256:8d7c8a87be0e06d176943bf5d873c80f11d670999518c4bca15a2a82de5c6290`
- config `User`: `0`
- config `Entrypoint`: `/usr/local/bin/jain-supervisor`
- config `Cmd`: `serve`
- config ports: `8080/tcp`, `8888/tcp`
- config healthcheck: `null`
- Jain provenance labels: absent (only base-image labels were observed)
- cosign verification: valid with the published `cosign.pub`, as captured in the prior audit

The valid signature proves who signed those exact unsafe bytes; it does not make their runtime
contract acceptable.

### Newer local role images (not registry releases)

| Role/local name | Local image ID/apparent digest | Registry readback | Relevant config |
|---|---|---|---|
| `image.neverhuman.org/jain/web-control:build` | `sha256:3de339cabe02ee4c3e3f06422a311c77d0e1c028ed855b748137c79e8d61711b` | `NOT FOUND` | user `65534:65534`, port 8080, healthcheck present |
| `image.neverhuman.org/jain/scqd:build` | `sha256:2d6e160246731b4851d7a53987e78bd31e176c23d7a0e48949bcc3c6a7b070ca` | `NOT FOUND` | user `65532:65532`, port 7700, healthcheck present |
| `image.neverhuman.org/jain/worker-cpu:build` | `sha256:34746834fb55234b262e942abcdb533abd8ab2c348e10dd7845a35d683e7d878` | `NOT FOUND` | user `65532:65532`; stale probe healthcheck |
| `image.neverhuman.org/jain/worker-gpu:build` | `sha256:b300f61f523858559c1d3d40927db691ed345b9fd2e890d92e963c6e9d08d98b` | `NOT FOUND` | user `10001:10001`; stale probe healthcheck |

These values are local image IDs, not independently proven remote OCI manifest digests. They must
not be copied into a release Dockerfile. Scratch output contains SBOM/Grype material for an older
8.0.0/local-image build, but no canonical 8.0.1 receipt was found binding the four values above to
protected source, registry readback, SBOM, provenance, scan policy, and cosign bundles. In
particular, the observed GPU scan has unresolved vulnerability concerns and cannot be inferred
green from the local image's existence.

## Deploy source and PR snapshot

Read-only forge/source observations at audit time:

- protected `jain-deploy/main`: `dd80c5742fd1b1b7544078e1c1f6fef71e5e1a74`
- canonical checkout: clean `claude/release-testdata-20260714` at
  `429bfc7f7a33262533981552129f85e9cc006aa8` (the local tracking ref was stale and reported
  `ahead 3`; `ls-remote` returned the same live branch head)
- appliance consolidation branch / PR #26:
  `claude/appliance-v8.0.1-20260716` at
  `40e589852eed0bb82544626c4326141be149a6a4`, open and blocked
- guestd branch / PR #22: `claude/guestd-20260715` at
  `f8afefce3c0e5c7face360fde3f89205fdac1910`, open and blocked
- release-testdata / PR #23: head `429bfc7...`, open and blocked
- duplicate PRs #24 and #25: closed, not merged

PR #26 is the right consolidation lane for cloud-role source, but it does not by itself create a
standalone signed image or a protected, immutable customer Dockerfile. Do not merge or publish a
download merely because its compose rendering succeeds.

## Exact publication blockers, in dependency order

1. **Define the standalone artifact boundary.** Decide and document which protected source builds
   the customer image. Do not call the multi-container cloud topology a single-container
   appliance. Bind the standalone artifact to 8.0.1 source/tag/tree in the authority release spec.
2. **Land reviewed source.** The image Dockerfile/build recipe and all copied inputs must land via
   the protected `jain-deploy` lifecycle with green required CI, proof, independent approval, and
   protected merge. PR #26 is currently not merged.
3. **Build once from the merged/tagged SHA.** Produce the standalone image once with deterministic
   inputs. No rebuild is allowed between qualification and publication.
4. **Prove the runtime contract.** Inspect and run the exact digest. Require non-root, only port
   8080, a passing bounded healthcheck, fixed entrypoint, read-only root, tmpfs writable state,
   cap-drop, no-new-privileges, resource/log bounds, clean shutdown, and restart behavior. Run GPU
   allocation/release and prove GPU-required work cannot silently fall back to CPU.
5. **Close scan/provenance gates.** Generate and bind SBOM and SLSA-style provenance, run
   vulnerability and license policy, and resolve the known GPU-image findings. No scratch receipt
   or local Docker cache is release authority.
6. **Publish without moving an accepted tag.** Push to the intended public repository, then obtain
   the OCI index digest from independent registry readback. Never substitute a local image ID for
   this value.
7. **Sign and read back the exact digest.** Cosign the registry digest with the release owner key,
   verify it using the public key from an independent host, and bind the signature/bundle hash in
   `CloudReleaseSpecV1`.
8. **Render the one-line Dockerfile.** Replace the render token only after steps 1–7 pass. Reject
   any `ARG`, unpinned `FROM`, `COPY`, `ADD`, `RUN`, second stage, second port, or shell command.
9. **Protect and sign the file bytes.** Land the rendered bytes through a protected PR. Generate a
   per-file SHA-256 and a cosign blob bundle/signature for the exact bytes. The existing image
   signature is not a file signature.
10. **Publish an immutable versioned bundle.** Recommended URLs are:
    `https://downloads.neverhuman.org/jain/8.0.1/Dockerfile`,
    `Dockerfile.sha256`, `Dockerfile.bundle.json`, and `cosign.pub` in the same directory. Publish
    with an expected Caddy/object ETag and immutable cache policy. A stable alias may change only
    after the versioned URL passes readback; the 8.0.1 URL must never change afterward.
11. **Verify from an empty context and a clean host.** Fetch only the four public files, verify the
    file signature/checksum, build using an otherwise empty directory, inspect the result, run it
    under the security constraints, and wait for healthy. Prove that no local Jain checkout,
    vendor directory, secret, Docker socket, or `/etc/hosts` entry is used.
12. **Bind final acceptance evidence.** Record HTTP status/headers, exact file hashes, image
    digest/signature, clean-host build ID, runtime/health/GPU receipts, source PR/merge/tag/checks,
    and independent registry/URL readback in the 8.0.1 release snapshot before advertising it.

## Required verification commands

The commands below are the acceptance procedure after a valid digest is available. They contain no
private key material. Signing commands must run in the owner-controlled signer environment using a
KMS/key URI supplied there; never store a raw key or credential in this evidence tree.

### Owner-side image publication and proof

```sh
IMAGE='image.neverhuman.org/veox/jain@sha256:<64-lowercase-hex-registry-digest>'
docker buildx imagetools inspect "$IMAGE"
cosign sign --yes --key "$COSIGN_KMS_URI" "$IMAGE"
cosign verify --key cosign.pub "$IMAGE"
syft "$IMAGE" -o cyclonedx-json > standalone-gpu.sbom.cdx.json
grype "$IMAGE" -o json > standalone-gpu.grype.json
```

Release tooling, not a manual editor, then renders the one-line file and signs its bytes:

```sh
sha256sum Dockerfile > Dockerfile.sha256
cosign sign-blob --yes --key "$COSIGN_KMS_URI" --bundle Dockerfile.bundle.json Dockerfile
```

### Independent public readback

```sh
BASE='https://downloads.neverhuman.org/jain/8.0.1'
tmpdir="$(mktemp -d)"
curl --fail --show-error --silent --location "$BASE/Dockerfile" -o "$tmpdir/Dockerfile"
curl --fail --show-error --silent --location "$BASE/Dockerfile.sha256" -o "$tmpdir/Dockerfile.sha256"
curl --fail --show-error --silent --location "$BASE/Dockerfile.bundle.json" -o "$tmpdir/Dockerfile.bundle.json"
curl --fail --show-error --silent --location "$BASE/cosign.pub" -o "$tmpdir/cosign.pub"
cd "$tmpdir"
sha256sum --check Dockerfile.sha256
cosign verify-blob --key cosign.pub --bundle Dockerfile.bundle.json Dockerfile
test "$(wc -l < Dockerfile)" -eq 1
grep -Eq '^FROM image[.]neverhuman[.]org/veox/jain@sha256:[0-9a-f]{64}$' Dockerfile
! grep -Eq '^(ARG|ADD|COPY|RUN|USER|EXPOSE|HEALTHCHECK|ENTRYPOINT|CMD)[[:space:]]' Dockerfile
docker build --pull --no-cache --network=none --tag jain-8.0.1-download -f Dockerfile .
```

The final negative `grep` intentionally permits only `FROM`; any attempted wrapper repair fails
acceptance. `--network=none` applies to build steps; registry resolution of the sole base still
uses the Docker engine's registry transport.

### Image config and runtime checks

```sh
docker image inspect jain-8.0.1-download
docker run --detach --name jain-8.0.1-download \
  --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=256m \
  --cap-drop ALL \
  --security-opt no-new-privileges=true \
  --pids-limit 256 \
  --memory 8g \
  --cpus 4 \
  --gpus all \
  --publish 127.0.0.1:8080:8080 \
  jain-8.0.1-download
docker inspect --format '{{.Config.User}} {{json .Config.ExposedPorts}} {{json .Config.Healthcheck}}' jain-8.0.1-download
docker inspect --format '{{.State.Health.Status}}' jain-8.0.1-download
curl --fail --show-error --silent http://127.0.0.1:8080/health
docker stop --time 30 jain-8.0.1-download
docker rm jain-8.0.1-download
```

Acceptance requires a non-zero numeric user, exactly `8080/tcp`, a non-null bounded healthcheck,
`healthy` state, successful GPU exercise/release evidence, and no mounted Docker socket. The
generic commands above do not replace the dedicated GPU and fail-closed allocation tests.

## Terminal decision

**BLOCK.** The design is reduced to a safe one-line final contract, but its approved digest does
not exist yet. The next owner action is not to edit the public file; it is to land and qualify the
standalone 8.0.1 image, publish/sign/read it back, then render and protect the exact one-line
Dockerfile and its immutable versioned signature bundle.
