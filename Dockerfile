FROM rust:1.85 AS builder

WORKDIR /usr/src/clippy
COPY . .

RUN cargo install --path ./clippy-server --bin clippy-server

FROM debian:bullseye-slim

COPY --from=builder /usr/local/cargo/bin/clippy-server /usr/local/bin/clippy-server

EXPOSE 7777

CMD ["clippy-server"]
