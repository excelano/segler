#!/bin/sh
# Build the binary package DESIGN.md §8 ships through the Excelano apt
# repository: both executables, the desktop entry, the media types, the icons
# and the two manual pages. One package, `segler`, for one product.
#
# A binary package rather than a source package. Everything here is two static
# Rust executables and a handful of data files, and the archive is assembled
# from a staging tree the same way `dpkg-deb` would assemble it from
# `debian/rules`. The shape is slipcase-desktop's.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/../.." && pwd)
target_dir=""
outdir="${root}/dist"

usage() {
    cat <<'USAGE'
usage: build-deb.sh [--target-dir DIR] [--outdir DIR]

  --target-dir DIR  the Cargo target directory holding release/segler-desktop
                    and release/segler (default: ask cargo)
  --outdir DIR      where to write the .deb (default: ./dist)
USAGE
}

while [ $# -gt 0 ]; do
    case "$1" in
        --target-dir) target_dir="${2:?--target-dir needs a directory}"; shift 2 ;;
        --outdir) outdir="${2:?--outdir needs a directory}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "build-deb.sh: unknown argument $1" >&2; usage >&2; exit 2 ;;
    esac
done

# Cargo is asked where its target directory is. `[build] target-dir` in a Cargo
# configuration file moves it and no environment variable then says so.
if [ -z "$target_dir" ]; then
    target_dir=$(cd "$root" && cargo metadata --format-version 1 --no-deps |
        sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
fi
desktop="${target_dir}/release/segler-desktop"
cli="${target_dir}/release/segler"
for binary in "$desktop" "$cli"; do
    [ -x "$binary" ] || {
        echo "build-deb.sh: no executable at $binary — run 'cargo build --release --workspace' first" >&2
        exit 1
    }
done

# Through `version.sh`, which is the only thing here that reads Cargo.toml.
version=$("${here}/../version.sh")
# The changelog names the version being built, or the package ships release
# notes for something else. This check is what lets the changelog be
# hand-written: a file refused unless it matches cannot go stale.
changelog_version=$(sed -n '1s/^[^(]*(\([^)]*\)).*/\1/p' "${here}/changelog")
[ "$changelog_version" = "$version" ] || {
    echo "build-deb.sh: the changelog's newest entry is ${changelog_version:-unreadable}," \
         "and Cargo.toml says ${version}" >&2
    echo "build-deb.sh: add an entry to packaging/debian/changelog before building" >&2
    exit 1
}

arch=$(dpkg-architecture -qDEB_HOST_ARCH)

# The architecture the package declares has to be the architecture the
# executables actually are. `dpkg-architecture` answers about this machine,
# which is right for a native build and says nothing about a binary from a
# target directory handed over by hand; a package declaring amd64 while
# carrying an arm64 executable installs perfectly and then does not run.
#
# `e_machine` is two little-endian bytes at offset 18, after the four magic
# bytes have said this is an ELF at all. 62 is x86-64 and 183 is AArch64.
case "$arch" in
    amd64) want=62 ;;
    arm64) want=183 ;;
    *) want="" ;;
esac
for binary in "$desktop" "$cli"; do
    magic=$(od -An -tx1 -N4 "$binary" | tr -d ' \n')
    [ "$magic" = "7f454c46" ] || {
        echo "build-deb.sh: $binary is not an ELF executable" >&2
        exit 1
    }
    lo=$(od -An -tu1 -j18 -N1 "$binary" | tr -d ' ')
    hi=$(od -An -tu1 -j19 -N1 "$binary" | tr -d ' ')
    machine=$((lo + hi * 256))
    if [ -n "$want" ] && [ "$machine" != "$want" ]; then
        echo "build-deb.sh: this machine is ${arch}, which wants ELF machine ${want}," \
             "and ${binary} is machine ${machine}" >&2
        echo "build-deb.sh: build the package on the architecture it is for" >&2
        exit 1
    fi
done

name="segler_${version}_${arch}"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
# mktemp makes it 0700, and dpkg-deb records the staging root as the package's
# own `./`, so without this every install leaves an unreadable directory mode
# behind it.
chmod 0755 "$stage"

mkdir -p \
    "${stage}/DEBIAN" \
    "${stage}/usr/bin" \
    "${stage}/usr/share/applications" \
    "${stage}/usr/share/mime/packages" \
    "${stage}/usr/share/icons/hicolor/scalable/apps" \
    "${stage}/usr/share/icons/hicolor/scalable/mimetypes" \
    "${stage}/usr/share/man/man1" \
    "${stage}/usr/share/doc/segler"

install -m 0755 "$desktop" "${stage}/usr/bin/segler-desktop"
install -m 0755 "$cli" "${stage}/usr/bin/segler"
install -m 0644 "${here}/../linux/segler-desktop.desktop" \
    "${stage}/usr/share/applications/segler-desktop.desktop"
install -m 0644 "${here}/../linux/mime/doclang.xml" \
    "${stage}/usr/share/mime/packages/doclang.xml"
install -m 0644 "${here}/../linux/icons/segler-desktop.svg" \
    "${stage}/usr/share/icons/hicolor/scalable/apps/segler-desktop.svg"
for type in application-vnd.doclang.archive+zip application-vnd.doclang.document+xml; do
    install -m 0644 "${here}/../linux/icons/${type}.svg" \
        "${stage}/usr/share/icons/hicolor/scalable/mimetypes/${type}.svg"
done
# Debian's own shape rather than the repository's LICENSE. Policy §12.5 wants
# the Apache text referred to at /usr/share/common-licenses rather than
# copied, and a copyright notice with a year and a holder; lintian makes both
# errors. slipcase-desktop ships its MIT text verbatim and passes, because
# MIT is not among the common licences and has no such rule.
install -m 0644 "${here}/copyright" "${stage}/usr/share/doc/segler/copyright"

# Debian policy wants a changelog in every binary package, and lintian makes
# its absence an error: somebody installing from an apt repository has no other
# way to see what changed between two versions. Hand-written rather than
# derived from `git log`: a commit subject is written for whoever maintains
# this, and the changelog for whoever is deciding whether to upgrade.
#
# `-n` so the compressed copy carries no name or timestamp of its own, which is
# what makes two builds of the same source produce the same bytes.
gzip -9nc "${here}/changelog" > "${stage}/usr/share/doc/segler/changelog.gz"
chmod 0644 "${stage}/usr/share/doc/segler/changelog.gz"

for page in segler-desktop segler; do
    sed "s/@VERSION@/${version}/" "${here}/${page}.1.in" \
        | gzip -9nc > "${stage}/usr/share/man/man1/${page}.1.gz"
    chmod 0644 "${stage}/usr/share/man/man1/${page}.1.gz"
done

# Stripped here rather than by the build profile, so a developer's release
# binary keeps its symbols and only the packaged copy loses them.
strip --strip-unneeded "${stage}/usr/bin/segler-desktop" "${stage}/usr/bin/segler" 2>/dev/null || true

# The umask of whoever ran this is not a packaging decision. `install -m`
# already fixed every file; this fixes the directories they sit in.
find "$stage" -type d -exec chmod 0755 {} +

sed -e "s/@VERSION@/${version}/" -e "s/@ARCH@/${arch}/" \
    -e "s/@SIZE@/$(du -ks "${stage}/usr" | cut -f1)/" \
    "${here}/control.in" > "${stage}/DEBIAN/control"

# `dpkg -V` verifies an installed copy against this file, and lintian tags its
# absence. Generated after the strip above, so the hash recorded is the hash of
# the binary that ships.
(
    cd "$stage"
    find . -type f ! -path './DEBIAN/*' -printf '%P\0' \
        | sort -z \
        | xargs -0 --no-run-if-empty md5sum > DEBIAN/md5sums
)
chmod 0644 "${stage}/DEBIAN/md5sums"

mkdir -p "$outdir"
dpkg-deb --root-owner-group --build "$stage" "${outdir}/${name}.deb" >/dev/null
echo "${outdir}/${name}.deb"

# What the executable links, against what the package declares. Most of what
# the window needs is opened by name at run time and appears in neither list,
# which is why `Depends` is written by hand and why this prints the comparison
# rather than deriving one from the other.
echo
echo "linked at load time:"
objdump -p "$desktop" | awk '/NEEDED/ {print "  " $2}'
echo "declared in Depends:"
sed -n 's/^Depends: //p' "${stage}/DEBIAN/control" | tr ',' '\n' | sed 's/^ */  /'
