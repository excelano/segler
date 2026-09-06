# The document a reviewer opens

Both stores put a tester in front of the application with no DocLang file,
and an editor with nothing to open looks like it does nothing. So each
submission's review notes name a document to download and open, and each
lane's screenshots are taken from the same one, so that what the tester sees
is what the listing shows.

**What belongs here: `sailing-directions.dclx`**, the two-page archive the
Windows lane wrote for its screenshots on 2026-09-05, *Sailing directions for
the western approaches*: a heading hierarchy, running text with bold and
italic, a four-column table with a header row and a caption, an ordered and
an unordered list, a picture with a caption and its own inner text, and two
page images drawn to match. Every feature the listing claims is visible in
it and none of it is anybody else's copyright. It was in the Windows VM's
`dist/screenshots/` and nowhere else, which is why this directory exists:
a file three store submissions point at should not live in an ignored
directory on one machine. It was committed here from that machine on
2026-09-06.

Once it is here it is served from GitHub and nowhere else. The release
step attaches it to the GitHub release beside the `.deb`, so its address is
pinned to the version the screenshots were taken from:

    https://github.com/excelano/segler/releases/download/vX.Y.Z/sailing-directions.dclx

That URL goes in both stores' review notes and in `store-listing.md`'s
screenshot section as the document used. Nothing is copied to a website:
a file for two reviewers and the odd visitor has no need of one, and a copy
somewhere else is a second thing to keep in step.

**Until the release exists, the review notes point at the DocLang project's
own sample**, `https://www.doclang.ai/viewer/assets/2501.17887.dclx`, the
archive the hosted viewer loads as its demo and the one both keyboard
walkthroughs used. It is 8 MB, carries page images, and is served by the
project rather than by us, which is a fine thing for a reviewer to open and
a poor thing to depend on: it can move or change without anyone here
knowing. Replace it with the URL above when the tag is cut and the release
carries the file.

## What is in `source/`

The archive was not written by hand, so what made it is committed beside it
and it can be made again:

| | |
| --- | --- |
| `sailing-directions.dclg` | The document, which is `document.xml` in the archive |
| `pages/1.png`, `pages/2.png` | The two page images, 1000x1400, the `default_resolution` the document declares |
| `assets/figure.png` | The tide curve the `<picture>` points at |
| `make-pages.ps1`, `make-figure.ps1` | What drew those three, with `System.Drawing` |
| `pack.ps1` | What packs the six parts into the `.dclx` |

`pack.ps1` alone rebuilds the archive from what is committed. The two drawing
scripts need Windows and the Georgia font, and GDI+ text metrics are not
identical from machine to machine, which is why their output is committed
rather than left to be regenerated: the images in the archive are the ones
the screenshots were taken against. On the machine that drew them a rerun is
byte-identical, which is what says the scripts and the images are the same
three pictures and not two versions of them.

Deflate is not reproducible across implementations, so a fresh pack is equal
to the committed archive member by member and not byte for byte. That was
checked when these landed - six members, same names, same order, same bytes -
and `pack.ps1 -Out somewhere-else.dclx` is how to check it again without
overwriting anything.

Author: David M. Anderson
Built with AI assistance (Claude, Anthropic)
