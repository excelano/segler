#!/bin/sh
# Install the desktop integration DESIGN.md §8 describes: the two media types,
# the desktop entry, and the three icons. Optionally the executables alongside
# them.
#
# For a person installing by hand and for testing the association without
# building a package. The Excelano apt repository ships the same files from
# `packaging/debian`, and the two must agree about where things go.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix="${HOME}/.local"
binaries=""
target_dir=""

usage() {
    cat <<'USAGE'
usage: install.sh [--prefix DIR] [--target-dir DIR] [--no-binary]

  --prefix DIR      where to install (default: ~/.local; use /usr/local for all users)
  --target-dir DIR  the Cargo target directory to take the executables from
  --no-binary       install the desktop integration only

Without --no-binary, segler-desktop and segler are looked for under the target
directory, release before debug, and installed into PREFIX/bin if found.
USAGE
}

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) prefix="${2:?--prefix needs a directory}"; shift 2 ;;
        --target-dir) target_dir="${2:?--target-dir needs a directory}"; shift 2 ;;
        --no-binary) binaries="none"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "install.sh: unknown argument $1" >&2; usage >&2; exit 2 ;;
    esac
done

# Cargo is asked where its target directory is rather than guessed at, because
# `[build] target-dir` in a Cargo configuration file moves it and no environment
# variable then says so.
if [ "$binaries" != "none" ]; then
    if [ -z "$target_dir" ] && command -v cargo >/dev/null 2>&1; then
        target_dir=$(cd "${here}/../.." && cargo metadata --format-version 1 --no-deps 2>/dev/null |
            sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
    fi
    [ -n "$target_dir" ] || target_dir="${here}/../../target"
fi

mkdir -p \
    "${prefix}/share/applications" \
    "${prefix}/share/mime/packages" \
    "${prefix}/share/icons/hicolor/scalable/apps" \
    "${prefix}/share/icons/hicolor/scalable/mimetypes"

install -m 0644 "${here}/segler-desktop.desktop" \
    "${prefix}/share/applications/segler-desktop.desktop"
install -m 0644 "${here}/mime/doclang.xml" \
    "${prefix}/share/mime/packages/doclang.xml"
install -m 0644 "${here}/icons/segler-desktop.svg" \
    "${prefix}/share/icons/hicolor/scalable/apps/segler-desktop.svg"
for type in application-vnd.doclang.archive+zip application-vnd.doclang.document+xml; do
    install -m 0644 "${here}/icons/${type}.svg" \
        "${prefix}/share/icons/hicolor/scalable/mimetypes/${type}.svg"
done

if [ "$binaries" != "none" ]; then
    for name in segler-desktop segler; do
        found=""
        for candidate in "${target_dir}/release/${name}" "${target_dir}/debug/${name}"; do
            if [ -x "$candidate" ]; then found="$candidate"; break; fi
        done
        if [ -n "$found" ]; then
            mkdir -p "${prefix}/bin"
            install -m 0755 "$found" "${prefix}/bin/${name}"
            echo "installed ${prefix}/bin/${name} from ${found}"
        else
            echo "no ${name} under ${target_dir}; it must be on PATH for the entry to work"
            echo "  (GLib drops a desktop entry whose Exec it cannot find, so until it is," \
                 "the entry is not registered and the file manager offers nothing)"
        fi
    done
fi

# Each is absent on a minimal system and each failure is survivable: the files
# are in place either way and the next login or the next package installation
# rebuilds these caches.
[ -x "$(command -v update-mime-database || true)" ] &&
    update-mime-database "${prefix}/share/mime" >/dev/null 2>&1 || true
[ -x "$(command -v update-desktop-database || true)" ] &&
    update-desktop-database "${prefix}/share/applications" || true
[ -x "$(command -v gtk-update-icon-cache || true)" ] &&
    gtk-update-icon-cache -q -t -f "${prefix}/share/icons/hicolor" || true

echo "installed the Segler desktop entry, the DocLang media types and the icons under ${prefix}"
echo
echo "check it with:"
echo "  gio info -a standard::content-type SOME.dclx   # application/vnd.doclang.archive+zip"
echo "  gio mime application/vnd.doclang.archive+zip   # segler-desktop.desktop"
