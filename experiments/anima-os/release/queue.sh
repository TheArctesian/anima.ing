#!/bin/sh
# Called by the conductor on dev; workflow_dispatch survives GITHUB_TOKEN fan-out rules.
set -eu
repo=${GITHUB_REPOSITORY:-TheArctesian/anima.ing}
enabled=$(gh api "repos/$repo/actions/variables/ANIMA_OS_RELEASES" --jq .value 2>/dev/null || true)
[ "$enabled" = on ] || exit 0
sha=$(git ls-remote origin refs/heads/dev | cut -f1)
count=$(gh run list --repo "$repo" --workflow os-release.yml --branch dev --commit "$sha" --limit 20 \
    --json status,conclusion --jq '[.[] | select(.conclusion != "skipped" and .conclusion != "cancelled")] | length')
[ "$count" != 0 ] || gh workflow run os-release.yml --repo "$repo" --ref dev
