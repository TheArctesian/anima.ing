#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
root="$(git rev-parse --show-toplevel)"
common="$(git rev-parse --path-format=absolute --git-common-dir)"
rust_version="$(sed -n 's/^channel *= *"\([^"]*\)"/\1/p' rust-toolchain.toml)"
go_version="$(sed -n 's/^go \([0-9.]*\)$/\1/p' link/go.mod)"
[[ "$rust_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$go_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "Expected exact Rust and Go toolchain versions." >&2
  exit 1
}
cache="$(printf %s "$root" | shasum -a 256 | cut -c1-20)"
builder="anima-os-runtime-builder:rust${rust_version}-go${go_version}"
docker build --platform linux/amd64 \
  --build-arg RUST_VERSION="$rust_version" --build-arg GO_VERSION="$go_version" \
  -t "$builder" - < os/release/runtime-builder.Dockerfile
mkdir -p os/payload target
docker run --rm --init --platform linux/amd64 \
  --mount "type=bind,source=$root,target=/src,readonly" \
  --mount "type=bind,source=$common,target=$common,readonly" \
  --mount "type=bind,source=$root/os/payload,target=/src/os/payload" \
  -v "anima-os-runtime-target-$cache:/src/target" \
  -v anima-linux-cargo:/usr/local/cargo/registry \
  -v anima-os-runtime-go:/go/pkg/mod \
  -v "anima-os-runtime-go-build-$cache:/go/cache" \
  -e GOCACHE=/go/cache -e GOPATH=/go -e RUSTC_WRAPPER= \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/src \
  -w /src "$builder" bash os/release/prepare-runtime.sh
