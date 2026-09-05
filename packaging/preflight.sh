#!/bin/sh
# Everything that must be true before a release is built, asked at once.
#
# slipcase-desktop wrote this after doing these checks by hand for four
# releases in a day, and doing them by hand is what gets skipped on the patch
# nobody thinks is risky. Each one below has been the thing that went wrong at
# least once over there.
#
#     ./packaging/preflight.sh                         # everything it can check locally
#     ./packaging/preflight.sh --corpus ~/clones/doclang --ci
#
# It refuses; it does not repair. A check that quietly fixed what it found would
# be a release nobody looked at.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/.." && pwd)
cd "$root"

ask_ci=no
corpus=""
while [ $# -gt 0 ]; do
    case "$1" in
        --ci) ask_ci=yes; shift ;;
        --corpus) corpus="${2:?--corpus needs a directory}"; shift 2 ;;
        -h|--help)
            sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "preflight.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

failed=0
say() { printf '  %-50s %s\n' "$1" "$2"; }
ok()   { say "$1" "ok"; }
bad()  { say "$1" "NO — $2"; failed=$((failed + 1)); }

version=$("${here}/version.sh")
echo "segler ${version}"

# 1. A release is cut from what is committed. A dirty tree means the artefact
#    and the tag describe different code, and nothing afterwards can tell.
if [ -z "$(git status --porcelain)" ]; then
    ok "the tree is clean"
else
    bad "the tree is clean" "$(git status --porcelain | wc -l | tr -d ' ') uncommitted files"
fi

# 2. A tag on an unpushed commit points at nothing anybody else can fetch.
if [ -z "$(git log --oneline '@{u}..' 2>/dev/null)" ]; then
    ok "nothing unpushed"
else
    bad "nothing unpushed" "$(git log --oneline '@{u}..' | wc -l | tr -d ' ') commits"
fi

# 3. The Debian changelog is hand-written and can name a different version.
#    `build-deb.sh` refuses on that; finding out here is cheaper.
deb_version=$(sed -n '1s/^[^(]*(\([^)]*\)).*/\1/p' packaging/debian/changelog)
if [ "$deb_version" = "$version" ]; then
    ok "packaging/debian/changelog names ${version}"
else
    bad "packaging/debian/changelog names ${version}" "it names ${deb_version:-nothing}"
fi

# 4. The store notes are written from CHANGELOG.md, so a version with no
#    section there ships empty release notes rather than failing.
if grep -q "^## \[${version}\]" CHANGELOG.md; then
    ok "CHANGELOG.md has a ${version} section"
else
    bad "CHANGELOG.md has a ${version} section" "no heading for it"
fi

# 5. The numeric spellings refuse a version they cannot represent. Ask now
#    rather than on the machine that builds the MSIX.
if "${here}/version.sh" --appx >/dev/null 2>&1; then
    ok "the version has an AppxManifest spelling"
else
    bad "the version has an AppxManifest spelling" "$("${here}/version.sh" --appx 2>&1 | head -1)"
fi

# 6. CLAUDE.md says clippy must be silent and CI enforces it with -D warnings.
#    Local `cargo check` does not.
if cargo clippy --quiet --workspace --all-targets 2>&1 | grep -qE '^(warning|error)'; then
    bad "clippy is silent" "it is not"
else
    ok "clippy is silent"
fi

if cargo fmt --check >/dev/null 2>&1; then
    ok "the tree is formatted"
else
    bad "the tree is formatted" "cargo fmt --check disagrees"
fi

if cargo test --quiet --workspace >/dev/null 2>&1; then
    ok "the suite passes"
else
    bad "the suite passes" "it does not"
fi

# 7. The conformance corpus is a command and never a test, so nothing else
#    runs it. Skipped rather than failed when the checkout is not named.
if [ -n "$corpus" ]; then
    if cargo run --quiet -p segler -- corpus "$corpus" >/dev/null 2>&1; then
        ok "the conformance corpus round-trips and validates"
    else
        bad "the conformance corpus round-trips and validates" "it does not"
    fi
else
    say "the conformance corpus" "skipped — pass --corpus DIR"
fi

# 8. Green on this commit, not on some commit. Asked of GitHub because nothing
#    local knows. Three states: a run still going is neither a pass nor a
#    failure, and a release cut while CI is mid-flight is one nobody checked.
if [ "$ask_ci" = yes ]; then
    sha=$(git rev-parse HEAD)
    if ! command -v gh >/dev/null 2>&1; then
        bad "CI is green on HEAD" "no gh to ask with"
    else
        counts=$(gh run list --limit 20 --json headSha,status,conclusion \
            --jq "[.[] | select(.headSha == \"${sha}\")]
                  | \"\(length) \(map(select(.status != \"completed\")) | length) \(map(select(.status == \"completed\" and .conclusion != \"success\")) | length)\"" \
            2>/dev/null) || counts=""
        set -- ${counts:-0 0 0}
        total=$1 running=$2 red=$3
        if [ "$total" -eq 0 ]; then
            bad "CI is green on HEAD" "no runs for ${sha}"
        elif [ "$red" -gt 0 ]; then
            bad "CI is green on HEAD" "${red} of ${total} failed"
        elif [ "$running" -gt 0 ]; then
            bad "CI is green on HEAD" "${running} of ${total} still running"
        else
            ok "CI is green on HEAD (${total} runs)"
        fi
    fi
else
    say "CI is green on HEAD" "skipped — pass --ci"
fi

echo
if [ "$failed" -eq 0 ]; then
    echo "Nothing is stopping a release of ${version}."
else
    echo "${failed} check(s) failed. Nothing was changed."
    exit 1
fi
