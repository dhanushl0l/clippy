FROM rust:1.85 AS builder

WORKDIR /usr/src/clippy
COPY . .

RUN cargo install --path ./clippy-server --bin clippy-server

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/clippy-server /usr/local/bin/clippy-server

EXPOSE 7777

CMD ["clippy-server"]