# Central RedlineDB server. Multi-stage: build redlinedb-server from redline-core, run it slim.
# Build context is the primary RedlineDB checkout; see docker-compose.yml.
FROM rust:1.95-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --locked --release -p redlinedb-server

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 redline
COPY --from=build /src/target/release/redlinedb-server /usr/local/bin/redlinedb-server
USER redline
VOLUME ["/data"]
EXPOSE 6033
ENTRYPOINT ["/usr/local/bin/redlinedb-server"]
CMD ["--database", "/data/central.redline", "--listen", "0.0.0.0:6033"]
