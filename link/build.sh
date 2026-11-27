#!/bin/sh
# Builds the anima-link release programs with the build tags of Tailcat's own releases.
# deploy/bin/windows-bundle.ps1 builds the Windows application's amd64 program with the same tags (HMD-050).
set -eu
cd "$(dirname "$0")"
LIMIT_BYTES=20000000
# With an empty module cache `go list -m` prints no folder.
go mod download github.com/tailscale/tailcat
tags_file="$(go list -m -f '{{.Dir}}' github.com/tailscale/tailcat)/build-tags.txt"
[ -s "$tags_file" ] || { echo "link/build.sh: the build tags of Tailcat are not at $tags_file" >&2; exit 1; }
tags=$(tr '\n' ',' <"$tags_file" | sed 's/,$//')
case ",$tags," in *,ts_omit_portmapper,*) ;; *) echo "link/build.sh: the build tags do not leave out the port mapper" >&2; exit 1 ;; esac
mkdir -p bin
for target in darwin/arm64 darwin/amd64 linux/amd64 linux/arm64 windows/amd64 windows/arm64; do
  os=${target%/*} arch=${target#*/}
  suffix=""
  [ "$os" = windows ] && suffix=".exe"
  out="bin/anima-link-$os-$arch$suffix"
  case "$(uname -s)/$(uname -m)/$target" in
    Darwin/arm64/darwin/arm64)
      CGO_ENABLED=1 GOOS=$os GOARCH=$arch go build -trimpath -ldflags="-s -w" -tags "$tags" -o "$out" ./cmd/anima-link ;;
    *)
      GOOS=$os GOARCH=$arch go build -trimpath -ldflags="-s -w" -tags "$tags" -o "$out" ./cmd/anima-link ;;
  esac
  size=$(wc -c <"$out" | tr -d ' ')
  echo "$out: $size bytes"
  if [ "$size" -gt "$LIMIT_BYTES" ]; then
    echo "$out is larger than $LIMIT_BYTES bytes" >&2
    exit 1
  fi
done
