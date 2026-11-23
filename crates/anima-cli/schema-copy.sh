#!/bin/sh
# Copies the K1 schema into the package (INS-011); contracts/schema.graphql is the only source (PLN-016).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
cp "$HERE/../../contracts/schema.graphql" "$HERE/schema.graphql"
