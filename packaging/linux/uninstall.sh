#!/bin/sh
# Remove what install.sh put in place, and rebuild the caches that indexed it.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

prefix="${HOME}/.local"
keep_binary=""

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) prefix="${2:?--prefix needs a directory}"; shift 2 ;;
        --keep-binary) keep_binary=yes; shift ;;
        -h|--help)
            echo "usage: uninstall.sh [--prefix DIR] [--keep-binary]"; exit 0 ;;
        *) echo "uninstall.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

rm -f \
    "${prefix}/share/applications/segler-desktop.desktop" \
    "${prefix}/share/mime/packages/doclang.xml" \
    "${prefix}/share/icons/hicolor/scalable/apps/segler-desktop.svg" \
    "${prefix}/share/icons/hicolor/scalable/mimetypes/application-vnd.doclang.archive+zip.svg" \
    "${prefix}/share/icons/hicolor/scalable/mimetypes/application-vnd.doclang.document+xml.svg"

[ -n "$keep_binary" ] || rm -f "${prefix}/bin/segler-desktop" "${prefix}/bin/segler"

[ -x "$(command -v update-mime-database || true)" ] &&
    update-mime-database "${prefix}/share/mime" >/dev/null 2>&1 || true
[ -x "$(command -v update-desktop-database || true)" ] &&
    update-desktop-database "${prefix}/share/applications" || true
[ -x "$(command -v gtk-update-icon-cache || true)" ] &&
    gtk-update-icon-cache -q -t -f "${prefix}/share/icons/hicolor" || true

echo "removed the Segler desktop integration from ${prefix}"
