# The document a reviewer opens

Both stores put a tester in front of the application with no DocLang file,
and an editor with nothing to open looks like it does nothing. So each
submission's review notes name a document to download and open, and each
lane's screenshots are taken from the same one, so that what the tester sees
is what the listing shows.

**What belongs here: `sailing-directions.dclx`**, a two-page archive,
*Sailing directions for the western approaches*: a heading hierarchy, running
text with bold and italic, a four-column table with a header row and a
caption, an ordered and an unordered list, a picture with a caption and its
own inner text, and two page images drawn to match. Every feature the listing
claims is visible in it and none of it is anybody else's copyright.
`sailing-directions.de.dclx` is the German counterpart.

It is served from GitHub and nowhere else. `build-release` attaches it to
every release beside the `.deb`, so the newest copy is always at

    https://github.com/excelano/segler/releases/latest/download/sailing-directions.dclx

That URL goes in both stores' review notes in `store-listing.toml`. Nothing is
copied to a website: a file for two reviewers and the odd visitor has no need
of one, and a copy somewhere else is a second thing to keep in step.

## What is in `source/`

The archive was not written by hand, so what made it is committed beside it
and it can be made again:

| | |
| --- | --- |
| `sailing-directions.dclg`, `sailing-directions.de.dclg` | The document, which is `document.xml` in the archive - one file per language |
| `pages/1.png`, `pages/2.png` and their `.de` counterparts | The two page images, 1000x1400, the `default_resolution` the document declares |
| `assets/figure.png` and `figure.de.png` | The tide curve the `<picture>` points at, its axis label in the document's language |
| `make-pages.ps1`, `make-figure.ps1` | What drew those, with `System.Drawing`; `-Lang de` draws the German set |
| `pack.ps1` | What packs six parts into a `.dclx`; `-Lang de` packs the German six into `sailing-directions.de.dclx` |

`pack.ps1` alone rebuilds either archive from what is committed. The two
drawing scripts need Windows and the Georgia font, and GDI+ text metrics are
not identical from machine to machine, which is why their output is committed
rather than left to be regenerated: the images in each archive are the ones
the screenshots were taken against. On the machine that drew them a rerun is
byte-identical, which is what says the scripts and the images are the same
pictures and not two versions of them. Inside either archive the parts keep
the plain names (`document.xml`, `pages/1.png`, `assets/figure.png`, ...) -
the `.de` suffix is only how the two languages' sources tell each other apart
on disk before packing.

Deflate is not reproducible across implementations, so a fresh pack is equal
to the committed archive member by member and not byte for byte.
`pack.ps1 -Out somewhere-else.dclx` (or `-Lang de -Out ...`) is how to check
that without overwriting anything.
