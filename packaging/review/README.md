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
it and none of it is anybody else's copyright. It is in the Windows VM's
`dist/screenshots/` and nowhere else, which is why this directory exists:
a file three store submissions point at should not live in an ignored
directory on one machine.

Once it is here, copy it to `excelano.com/segler/sailing-directions.dclx`
and put that URL in both stores' review notes and in
`packaging/store-listing.md`'s screenshot section as the document used.

**Until it is here, the review notes point at the DocLang project's own
sample**, `https://www.doclang.ai/viewer/assets/2501.17887.dclx`, the
archive the hosted viewer loads as its demo and the one both keyboard
walkthroughs used. It is 8 MB, carries page images, and is served by the
project rather than by us, which is a fine thing for a reviewer to open and
a poor thing to depend on: it can move or change without anyone here
knowing. Replace it with the URL above when the file lands.

Author: David M. Anderson
Built with AI assistance (Claude, Anthropic)
