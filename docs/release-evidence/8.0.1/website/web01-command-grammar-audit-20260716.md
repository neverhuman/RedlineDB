# WEB-01 exact `/try` command grammar audit

- Claim: `codex-mcp-v801-web01-command-grammar-audit-20260716T060400Z`
- Audited at: `2026-07-16T06:13:07Z`
- Release: Jain `8.0.1` candidate
- Decision: **BLOCK** — protected Web `main` does not implement byte-exact command matching.
- Mutation boundary: source/forge readback only. No Web checkout edit, CI run, check publication,
  forge mutation, host/runtime/route mutation, public request, clone, or worktree action was made.

## Machine identity

The earlier `05:30Z` open-PR snapshot became stale during this audit. Fresh local-forge readback
proved that Web control PR #24 was protected-merged while the audit was running:

- `refs/heads/main = 13ea4968dde1286111dacf8cd0216fa71e931242`
- PR #24 `claude/web-control-20260715` head and merge commit:
  `13ea4968dde1286111dacf8cd0216fa71e931242`
- PR #24 state: `closed`, `merged=true`, merged at `2026-07-16T06:06:54.762500414Z`
- prior protected main: `6adcda48e5711dcf5934934236be09fcc959ff10`

Audited source identities on the merged head:

| Path | Git blob | rendered SHA-256 |
|---|---|---|
| `crates/jain-web-control/src/command.rs` | `c00f56542941b436e969d6c3c98b87b6e2a6a7e6` | `dcb8f40ff4410d032f3deb9a6c76254c944c4bc7ad3431afe0cca910f8109334` |
| `crates/jain-web-control/src/live.rs` | `7189da291b54d1e1970a810549983eb75257882e` | `80d5214114a3cafbbc8e1a4f0e38f1205aa7a8bebd5f6055187681351addff60` |
| `apps/web/src/lib/commands.ts` | `365c0316a7d5666c890ce96a89066e1e44bd688f` | `238d505ad87894de86dc8a5af86a50e181a75dc9569dff8c1f19dc40d7224c0e` |

All current open Web heads in the `05:30Z` snapshot except the now-merged control head carry the
same broad TypeScript parser blob as the old main. The only distinct `/try` implementation found
is the now-merged `jain-web-control` crate. Superseded PR #16 adds still more full-cockpit verbs but
does not add the bounded Rust `/try` parser.

## Public parser/handler inventory

### 1. Production-trial path: `jain-web-control` (the `/try` authority)

The merged Rust router exposes:

- `GET /try` -> static allocation page;
- `POST /api/public/allocations` -> anonymous lease grant;
- `GET /live/:slot/` -> inline command cockpit;
- `GET /live/:slot/ws` -> authenticated WebSocket command transport.

`/try` allocates a lease and redirects to `/live/gNN/`. The inline JavaScript submits
`{"frame":"command","line":...}` to `/live/gNN/ws`. The Rust WS session authenticates the lease,
checks slot and epoch, deserializes `ClientFrame`, and calls `line.parse::<RemoteCommand>()`.
Rejected commands produce `invalid_command` and do not enter the event ring. Accepted commands are
converted back to the closed enum's static spelling before they enter the ring. This is the public
trial authority; it does not import or call the TypeScript full-cockpit parser.

### 2. Full product cockpit: `apps/web/src/lib/commands.ts`

The Vite application imports `parseCommand` from both `App.tsx` and `useRunActions.ts`. It is a
different, much broader product UI and must never be routed as the production `/try` authority.
It trims input, makes the leading slash optional, lowercases the verb, accepts aliases and
arguments, and maps commands to typed HTTP/API actions. Its known verbs include `approve`,
`target`, `effort`, `start`, `next`, `continue`, `stop`, `cancel`, `restart`, `rerun`, `predict`,
`files`, `upload`, `repo`, `checkpoint`, `history`, `revert`, `rollback`, `defaults`, `set`,
`reset`, `help`, and `note`. Bare unrecognized text becomes a local note.

This parser is valid only as a non-trial product surface. Caddy/source acceptance must prove that
the final `/try`, `/api/public/*`, and `/live/gNN/*` routes reach `jain-web-control`, never the Vite
cockpit or its session APIs.

### 3. Operator CLI is not a public command channel

`crates/jain-web-control/src/main.rs` has Clap-only process startup options `--host`, `--port`, and
`--probe`. They are parsed once from the container/service command line and are not reachable from
HTTP, WebSocket frames, or `/try`. The only `std::process` use in the whole control crate is
`std::process::exit(run_probe(url))` for the container health probe.

An exact-head search found no `std::process::Command`, `tokio::process`, `Command::new`, PTY API,
`docker.sock`, shell invocation, public stdin attachment, or argv forwarding in
`crates/jain-web-control`. The public path therefore has no process-execution primitive today.

## Exact behavior proved from merged source and tests

### Rust `RemoteCommand` parser

The parser starts with `input.trim()`, then splits on any Unicode whitespace and trims the
remainder. Its own test `surrounding_whitespace_is_the_only_forgiveness` requires
`"  /stop\n"` and `"\t/next "` to succeed. Consequently, it is deliberately not byte-exact.

| Input class | Current direct-WS result | Required result |
|---|---|---|
| `/start`, `/next`, `/stop` | accepted | accepted |
| `start`, `next`, `stop` | rejected | rejected |
| `/START`, `/Start`, mixed case | rejected | rejected |
| `/continue`, `/cancel`, `/restart`, prefixes/suffixes | rejected | rejected |
| `/start now`, `/stop --force` | rejected | rejected |
| leading/trailing ASCII whitespace | **accepted** | **rejected** |
| leading/trailing Rust Unicode whitespace (for example NBSP/U+00A0, EM SPACE/U+2003, IDEOGRAPHIC SPACE/U+3000) | **accepted** | **rejected** |
| a valid command followed only by whitespace | **accepted** | **rejected** |
| NUL, zero-width characters, homoglyph slash, percent text | rejected | rejected |

### Browser normalization adds a second exactness failure

The inline `/live` page runs `const v = line.value.trim()` before sending. A user who types
`" /start "`, `"/stop\n"`, or a command wrapped in JavaScript-trimmable Unicode whitespace has
that distinct input normalized to a valid command before the server can reject it. Fixing only
the Rust parser is therefore insufficient; the inline JavaScript must stop trimming and must
locally compare the unchanged input against the three literal strings.

### Frame-shape and size hardening gaps

- `ClientFrame` is an internally tagged Serde enum whose source comment explicitly says unknown
  extra fields are tolerated. A frame containing an exact command plus an `argv`, `admin`, or other
  unrecognized field is currently accepted as a command frame. The extra data is ignored and is
  not executed, but the WebDelegate contract should reject the entire shape fail-closed.
- There is no application-level inbound text length check before `serde_json::from_str`. The
  parser bounds reflected command fragments to 32 characters, but a large WebSocket/JSON payload
  is still accepted into the deserializer first. Configure a small WS message/frame limit and an
  explicit pre-deserialization byte limit.
- Parse errors currently reflect a bounded fragment of the rejected verb/arguments. Prefer one
  constant typed `invalid_command` detail so no raw user input ever becomes protocol/log material.
- Binary frames are correctly rejected as `invalid_frame`; unknown frame tags are correctly
  rejected. Ping/Pong are transport-only.

## Cookie, lease, and command-side-effect boundary

Positive merged controls:

- lease cookie is `Secure; HttpOnly; SameSite=Strict; Path=/` with maximum age 60 minutes;
- slot policy constants are 15-minute idle and 60-minute absolute lifetime;
- allocation is idempotent per browser visitor and per valid lease;
- a WS upgrade is slot/cookie scoped and every inbound frame re-proves slot plus monotonic epoch;
- a revoked, expired, wrong-slot, or stale-epoch lease cannot enqueue a command;
- rejected command input does not enter the bounded event ring;
- no local CPU/process fallback exists in this crate; accepted commands currently report honest
  `unavailable` because no cluster is attached.

Remaining adjacent WEB-02 decisions/tests:

- there is no explicit WebSocket `Origin` allowlist in the audited handler;
- `pool.touch()` refreshes the idle deadline before JSON/frame/command validation, so authenticated
  malformed traffic can refresh idle time (never the 60-minute absolute deadline). Decide and
  test whether only a valid command/resume/ack counts as activity; if so, split non-mutating
  lease revalidation from the later touch;
- prove the visitor cookie has equally strict attributes and prove no credential reaches URL,
  body, client storage, or logs.

These are not authorization to broaden WEB-01; they are explicit boundaries for the corrective PR.

## Smallest fail-closed corrective implementation

1. Replace `FromStr`'s normalization/splitting logic with one exact byte comparison. The authority
   should be `TryFrom<&[u8]> for RemoteCommand` (with `FromStr` delegating to `input.as_bytes()` only
   if compatibility requires it):

   ```rust
   match input {
       b"/start" => Ok(RemoteCommand::Start),
       b"/next" => Ok(RemoteCommand::Next),
       b"/stop" => Ok(RemoteCommand::Stop),
       _ => Err(CommandParseError),
   }
   ```

   Do not trim, split, lowercase, Unicode-normalize, percent-decode, tokenize as argv, or add an
   alias registry. Use one non-reflecting error variant.
2. In the inline `/live` JavaScript, preserve `line.value` exactly. Compare it against a frozen
   three-string allowlist; reject anything else locally and do not send it. The Rust comparison
   remains the security authority.
3. Reject command frames with any field other than exact `frame` and `line`; likewise retain strict
   per-variant fields for `resume` and `ack`. Add a regression for ignored `argv`, `admin`,
   `command`, `input`, and duplicate-key shapes.
4. Bound the WS message and frame sizes at upgrade and check text length before JSON decoding.
   A command value needs at most six bytes; allow only the small fixed JSON-envelope budget needed
   by the three client frame variants.
5. Forward only `RemoteCommand`, slot, lease, and epoch to any future guestd/gateway delegate API.
   Do not expose a `String`, byte buffer, stdin, PTY, argv vector, shell, container console, Docker
   socket, or process-spawn callback in that API.
6. Keep the Vite full-cockpit parser out of the public-trial artifact/route. Add a route/build
   regression proving `/try` renders the Rust control page and not the Vite composer.

Expected surgical files: `command.rs`, `live.rs`, `frames.rs`, and `ws_transport.rs`; no broad
Web rewrite is needed.

## Required negative matrix

Unit-test success **iff** the input byte slice equals one member of
`[b"/start", b"/next", b"/stop"]`. The deterministic negative table must include:

- empty, `/`, missing slash, doubled slash, slash-space, trailing slash;
- every ASCII whitespace byte before, after, and within each valid command;
- CR, LF, CRLF, tab, vertical tab, and form feed;
- NBSP, EM SPACE, IDEOGRAPHIC SPACE, BOM, zero-width space/joiner;
- lowercase/uppercase/mixed-case mutations, prefixes, suffixes, and aliases;
- `/start now`, `/start --flag`, `/stop;sh`, pipes, redirects, quotes, backticks, `$()`, newline
  injection, embedded NUL, and multiple commands;
- percent-encoded slash/text, full-width slash, confusable/homoglyph letters, and normalization
  variants;
- invalid UTF-8 byte vectors at the byte-parser boundary, over-limit text, and a very large frame;
- JSON extra fields (`argv`, `admin`, `pty`, `stdin`, `docker`) and wrong/binary frame shapes.

Property tests:

- arbitrary `Vec<u8>` succeeds iff it is byte-equal to one of the three literals;
- deleting, inserting, prepending, appending, or changing any byte of a valid literal rejects;
- arbitrary bytes never panic or allocate proportional reflected error text;
- every rejected transport input leaves event-ring offset/capacity/SCQ dispatch unchanged;
- accepted commands are emitted only as the typed enum's canonical static spelling.

Integration tests must repeat the matrix through the real WS handler with a valid lease and prove
missing/wrong/stale cookie/slot/epoch frames cannot reach parsing or side effects. Browser tests
must type (not programmatically pre-normalize) whitespace, Unicode, case, missing-slash, alias, and
argv-injection forms and observe local rejection plus zero WS command frame.

## Acceptance and progress accounting

WEB-01 is **not complete**. Six of ten bounded controls are present: a distinct public parser, the
three canonical commands, rejection of missing slash/case/aliases/non-whitespace arguments, typed
drop with no enqueue, slot/epoch fencing, and no PTY/shell/process primitive. Four controls remain:
byte-exact whitespace rejection, browser preservation of exact input, strict frame fields, and
bounded inbound frames. This audit therefore assesses WEB-01 at **60% (6/10), still `[B]`**.

After a clean canonical-checkout handoff, the correction must use a branch freshly based on the
then-current protected `main`, receive focused Rust/WS/browser tests, exact-head required/coverage/
security/Jankurai proof, independent approval, and protected fast-forward merge. No test or source
change from this audit may be credited until that protected corrective lifecycle completes.
