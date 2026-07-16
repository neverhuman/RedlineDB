# Architecture

Redline Central is a standalone Rust workspace with two runtime boundaries:

- `redlinedb-client` owns framed TCP handshakes, typed wire values, statement
  lifecycle, and server error propagation.
- `db-shim` owns backend selection, namespace expansion, and transaction parity
  across embedded SQLite and the remote Redline client.

The Docker surface packages the separately governed RedlineDB server. This
repository does not copy or vendor sibling repository history. Consumers own
their application schemas and migrations; Redline Central owns only the
namespace and connection contract documented under `db/`.

Errors remain typed at both boundaries. Protocol errors never become successful
empty results, and unknown backend configuration fails closed. Required tests
use loopback TCP and in-memory SQLite so ambient services cannot affect proof.
