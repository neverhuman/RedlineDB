FROM rust:1.95.0-bookworm

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    procps \
    sqlite3 \
    strace \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /work
