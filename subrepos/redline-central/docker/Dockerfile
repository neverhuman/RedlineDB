# Central RedlineDB server. Multi-stage: build redlinedb-server from redline-core, run it slim.
# Build context must be redline-split/ (so redline-core is visible); see docker-compose.yml.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY redline-core ./redline-core
RUN cd redline-core && cargo build --release -p redlinedb-server

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 redline
COPY --from=build /src/redline-core/target/release/redlinedb-server /usr/local/bin/redlinedb-server
USER redline
VOLUME ["/data"]
EXPOSE 6033
ENTRYPOINT ["/usr/local/bin/redlinedb-server"]
CMD ["--database", "/data/central.redline", "--listen", "0.0.0.0:6033"]
