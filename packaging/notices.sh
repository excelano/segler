#!/usr/bin/env bash
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

set -euo pipefail
cd "$(dirname "$0")/.."

own=$(cargo metadata --no-deps --format-version 1 | jq -c '[.packages[].name]')

section() {
    printf '\n%s\n%s\n%s\n\n' "$(printf '=%.0s' {1..80})" "$1" "$(printf '=%.0s' {1..80})"
    tr -d '\r' < "$2"
}

crate_dir() {
    cargo metadata --format-version 1 \
        | jq -r --arg name "$1" '.packages[] | select(.name == $name) | .manifest_path' \
        | xargs dirname
}

{
    cargo about generate --fail -c packaging/about.toml --format json \
        | jq -r --argjson own "$own" '
            "Third-party software in this program, and the license each is used under.",
            (.licenses[]
                | .used_by |= map(select(.crate.name as $n | $own | index($n) | not))
                | select(.used_by | length > 0)
                | "", ("=" * 80), .name,
                  "Used by: " + ([.used_by[].crate | "\(.name) \(.version)"] | join(", ")),
                  ("=" * 80), "", .text)' \
        | tr -d '\r'
    if [ -f packaging/notices/extra.sh ]; then
        source packaging/notices/extra.sh
    fi
} > THIRD-PARTY-NOTICES.txt
