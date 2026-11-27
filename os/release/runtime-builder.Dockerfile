ARG GO_VERSION
ARG RUST_VERSION
FROM golang:${GO_VERSION}-bookworm AS go
FROM rust:${RUST_VERSION}-bookworm
COPY --from=go /usr/local/go /usr/local/go
ENV PATH=/usr/local/go/bin:/usr/local/cargo/bin:$PATH
