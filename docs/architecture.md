# Architecture

Redline Central is a standalone Rust workspace with two runtime boundaries:

- `redlinedb-client` owns framed TCP handshakes, typed wire values, statement
  lifecycle, and server error propagation.
- `db-shim` owns neutral values, errors, capabilities, structural parameter placement, validated
  namespace identifiers, and atomic transactions. Separate modules implement Redline, SQLite, and
  Postgres. Exactly one mutually exclusive feature selects an adapter at the composition root;
  applications contain no runtime named-backend branch.

The Docker surface packages the separately governed RedlineDB server. This
repository does not copy or vendor sibling repository history. Consumers own
their application schemas and migrations; Redline Central owns only the
namespace and connection contract documented under `db/`.

Errors remain typed at both boundaries. Protocol errors never become successful empty results.
Portable nulls explicitly retain their integer, real, text, or blob type; query projections declare
their portable result types so adapters never guess from an untyped null. Neutral execution reports
success only because every provider cannot prove an affected-row count. The governed operation
claim is `db-shim.used-operations/v2`, not arbitrary SQL parity. Local required proves the isolated
Cargo graphs and genuine in-memory SQLite behavior. The separate explicit family-release lane
requires live Redline and Postgres DSNs and fails closed rather than substituting a fake service;
protected PrivateNetwork host CI does not receive those credentials.
