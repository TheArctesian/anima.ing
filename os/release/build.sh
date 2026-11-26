#!/usr/bin/env bash
# Assemble a signed release or unsigned candidate from a detached Git checkout.
set -euo pipefail
usage() {
  echo "Usage: $0 [--unsigned] [--test-tools] <output-directory>" >&2
  echo "       $0 --rehearse-upgrade <pre-move-commit> <output-directory>" >&2
  exit 2
}

mode=signed
test_tools=false
baseline=
if [[ ${1:-} == --rehearse-upgrade ]]; then
  [[ $# == 3 ]] || usage
  mode=rehearsal
  baseline=$(git rev-parse --verify --end-of-options "$2^{commit}")
  shift 2
fi
if [[ ${1:-} == --unsigned ]]; then
  mode=unsigned
  shift
fi
if [[ ${1:-} == --test-tools ]]; then
  test_tools=true
  shift
fi
[[ $# == 1 ]] || usage
root=$(git rev-parse --show-toplevel)
revision=$(git rev-parse HEAD)
for input in os/release/assemble.sh os/modules/containers.nix os/console/workloads.js; do
  if ! git cat-file -e "$revision:$input" 2>/dev/null; then
    echo "Candidate $revision does not contain $input." >&2
    exit 1
  fi
done
if [[ "$mode" == rehearsal ]]; then
  if [[ "$baseline" == "$revision" ]] || ! git merge-base --is-ancestor "$baseline" "$revision"; then
    echo "The pre-move revision must be an ancestor of the candidate." >&2
    exit 1
  fi
  git cat-file -e "$baseline:experiments/anima-os/flake.nix"
  git cat-file -e "$revision:os/release/rehearsal.sh"
fi
mkdir -p -- "$1"
out=$(cd -- "$1" && pwd -P)
if [[ -n "$(find "$out" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  echo "Assembly requires an empty output directory: $out" >&2
  exit 1
fi
if [[ "$mode" == signed && ${OS_SIGNING_KEY:-} != /* ]]; then
  export OS_SIGNING_KEY="$PWD/${OS_SIGNING_KEY:?Set OS_SIGNING_KEY for a signed release}"
fi
scratch=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/anima-os-source.XXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
git clone --quiet --shared --no-checkout -- "$root" "$scratch/source"
git -C "$scratch/source" -c core.hooksPath=/dev/null checkout --quiet --detach "$revision"
cd "$scratch/source"
if [[ "$mode" == rehearsal ]]; then
  bash os/release/rehearsal.sh "$baseline" "$out" "$scratch"
else
  bash os/release/assemble.sh "$out" "$mode" "$test_tools"
fi
