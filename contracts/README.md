# Contracts

This directory owns RedlineDB's language-boundary contracts. The canonical,
hand-authored C ABI is under `contracts/c-abi/`; OpenAPI, JSON Schema, and
protobuf sources also belong here when introduced. Generated clients and
bindings must live only under paths declared in
`agent/generated-zones.toml`, and the required lane verifies drift between
each authored contract and its generated consumers.
