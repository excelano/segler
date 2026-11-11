# Submission notes

What a store submission needs from a person and no file supplies. The listing
text, Apple's Notes for Review and Microsoft's Notes for certification are
`store-listing.toml`, and what a release changed is `release-notes.toml`; the
release apps push both. A field edited here reaches nobody. The review notes
point at the document in `review/`, which every release carries.

## Capability justification

Partner Center asks once, when `runFullTrust` is first declared, and keeps the
answer against the product. 500 characters, newlines counted.

> Segler is a full-trust Win32 desktop application packaged as MSIX. It needs
> this capability to run at all. It opens the DocLang document it was
> launched with, or one chosen through Open, and writes only where a save
> dialog points. It makes no network connection of any kind, needs no broad
> filesystem access, and uses no device.

## The answers a form asks

    Category                  Productivity
    Copyright and trademark   Excelano LLC
    Additional license terms  blank; the application is Apache-2.0 and the
                              repository carries the license
    Privacy policy            https://excelano.com/legal/#segler
    App Privacy (Apple)       Data Not Collected

## Screenshots

The same four frames on both stores, taken by `shots-winstore` and
`shots-macstore` from the keyboard recipes in `windows/shots.ps1` and
`macos/shots.sh`, over the review document in each listing's language:
correcting a table cell, the page image beside the document, the correction
committed, and a picture on page two. Something is selected in every frame,
because a screenshot of an editor at rest shows a viewer. The Mac set repeats
the four in dark.
