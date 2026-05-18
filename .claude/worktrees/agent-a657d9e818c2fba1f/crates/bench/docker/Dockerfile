FROM rust:1.95.0-bookworm

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    procps \
    sqlite3 \
    strace \
    python3 \
    python3-pip \
  && python3 -m pip install --no-cache-dir --break-system-packages matplotlib numpy pandas \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /work
