#!/usr/bin/env bash
# Runs the tests of the thread runner and the holder (crates `anima-daemon` and `agent-threads`)
# on Linux in Docker, from a Mac, before a push of a change that touches processes. Linux reads
# `/proc` where a Mac reads `ps`, so a fault can pass on a Mac and fail on GitHub.
#
# The source is mounted read-only. The tests `units::` write into the source tree, so they are
# left out here; GitHub runs them. Two Docker volumes keep the build and the registry between runs.
# Each worktree has its own build folder in the volume: each is mounted at /src, and cargo would
# take the build of a different worktree for a source file that is older than it.
# Then, also when a test failed, it runs Clippy for Windows (the target of the job `windows-lint`) on the crates that hold
# code behind `cfg(windows)` near the runner: `anima-cli`, `anima-daemon` and `agent-threads`.
# `--no-windows` leaves that out.
# Arguments after `--` go to nextest (for example a filter of tests).
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
toolchain="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$root/rust-toolchain.toml")"
image="rust:${toolchain}-bookworm"
build="build-$(basename "$root")"
case "$(docker info --format '{{.Architecture}}')" in
  aarch64 | arm64) nextest_url="https://get.nexte.st/latest/linux-arm" ;;
  *) nextest_url="https://get.nexte.st/latest/linux" ;;
esac
windows=1
if [[ "${1:-}" == "--no-windows" ]]; then windows=0; shift; fi
if [[ "${1:-}" == "--" ]]; then shift; fi
extra=""
if [[ $# -gt 0 ]]; then extra="$(printf ' %q' "$@")"; fi

echo "linux threads: $image, crates anima-daemon and agent-threads, units:: left out"
docker run --rm \
  -v "$root":/src:ro \
  -v anima-linux-target:/target \
  -v anima-linux-cargo:/usr/local/cargo/registry \
  -e NEXTEST_URL="$nextest_url" \
  -e WINDOWS="$windows" \
  -w /src "$image" bash -c "
set -euo pipefail
if [[ ! -x /target/cargo-nextest ]]; then
  curl -LsSf \"\$NEXTEST_URL\" | tar -xzf - -C /target
fi
if [[ \$WINDOWS == 1 ]]; then
  command -v x86_64-w64-mingw32-gcc >/dev/null || { apt-get update -qq && apt-get install -y -qq mingw-w64 >/dev/null; }
  rustup target add x86_64-pc-windows-gnu >/dev/null
fi
id tester >/dev/null 2>&1 || useradd -m tester
chown -R tester /target /usr/local/cargo/registry /usr/local/rustup
su tester -s /bin/bash -c '
  set -euo pipefail
  cd /src
  export HOME=/home/tester PATH=/usr/local/cargo/bin:\$PATH CARGO_HOME=/usr/local/cargo
  export CARGO_TARGET_DIR=/target/$build RUSTC_WRAPPER= RUSTUP_HOME=/usr/local/rustup
  tests=0
  /target/cargo-nextest nextest run --cargo-quiet -p anima-daemon -p agent-threads \
    --no-fail-fast -E \"not test(/^units::/)\"$extra || tests=\$?
  if [[ $windows == 1 ]]; then
    echo \"windows lint: clippy --target x86_64-pc-windows-gnu, anima-cli, anima-daemon, agent-threads\"
    cargo clippy --locked --target x86_64-pc-windows-gnu -p anima-cli -p anima-daemon -p agent-threads \
      --all-targets -- -D warnings
  fi
  exit \$tests
'
"
