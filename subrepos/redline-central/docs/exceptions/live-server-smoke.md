# Live-server smoke exception

Owner: Redline Central protocol maintainers.

The required lane does not start or contact a shared RedlineDB server. Network
protocol behavior is covered by loopback integration tests; the real-server
smoke binary is scheduled only when an exact accepted Redline server artifact is
available. This prevents an ambient service from turning protected CI green.

Review trigger: any protocol-version, framing, authentication, or transport
change. Removal condition: a hermetic exact-artifact server fixture becomes part
of the standalone repository's local required lane.

Proof: `cargo test --locked -p redlinedb-client --test protocol_contract`.
