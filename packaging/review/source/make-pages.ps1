# Draw the two page images for `sailing-directions.dclx`.
#
# The archive's page images are what the *Page image* toggle shows beside the
# document, and their whole job is to be checkable: a line in the document
# pane and the same line on the scan, at the coordinates the document's
# <location> elements claim. So they are drawn here rather than photographed,
# at the `default_resolution` the document declares, 1000x1400.
#
# They are not a rendering of the document and are not meant to be one. A scan
# never matches its markup exactly, and a page image that did would hide the
# only thing the panel is for. The table and the two lists are drawn as flat
# text at roughly the right place, which is what a scan of them looks like.
#
# Needs Windows: `System.Drawing` and the Georgia font. GDI+ text metrics are
# not identical across machines, so a rerun elsewhere will not reproduce the
# committed PNGs byte for byte - which is why those are committed beside this
# script rather than left to be regenerated.
#
#   powershell -ExecutionPolicy Bypass -File packaging\review\source\make-pages.ps1
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$W = 1000; $H = 1400
$out = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'pages'
New-Item -ItemType Directory -Force -Path $out | Out-Null

function New-Page {
    param([int]$Number, [array]$Lines)
    $bmp = New-Object System.Drawing.Bitmap($W, $H)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.TextRenderingHint = 'AntiAliasGridFit'
    $g.Clear([System.Drawing.Color]::FromArgb(252, 251, 247))
    $ink = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(30, 30, 34))
    foreach ($l in $Lines) {
        $font = New-Object System.Drawing.Font('Georgia', $l.Size, $l.Style)
        $g.DrawString($l.Text, $font, $ink,
            (New-Object System.Drawing.RectangleF($l.X, $l.Y, ($W - $l.X - 80), 400)))
        $font.Dispose()
    }
    $small = New-Object System.Drawing.Font('Georgia', 11)
    $g.DrawString("$Number", $small, $ink, 490, 1330)
    $small.Dispose(); $ink.Dispose(); $g.Dispose()
    $path = Join-Path $out "$Number.png"
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "wrote $path"
}

$B = [System.Drawing.FontStyle]::Bold
$R = [System.Drawing.FontStyle]::Regular

New-Page 1 @(
    @{ Text='Sailing directions for the western approaches'; X=80; Y=88;  Size=22; Style=$B }
    @{ Text='The western approaches are the waters lying to the west of the channel, and this chapter describes them as a vessel meets them: the tides first, then the hazards, then the harbours of refuge.'; X=80; Y=168; Size=13; Style=$R }
    @{ Text='Tides'; X=80; Y=298; Size=17; Style=$B }
    @{ Text='Springs run hard along the whole of this coast. The table below gives the range at four ports, in metres, and the hour of high water after the standard port.'; X=80; Y=358; Size=13; Style=$R }
    @{ Text="Port        Springs     Neaps      HW`nFalmouth      5.3         2.4      -0:10`nNewlyn        5.6         2.6      -0:35`nPadstow       7.3         3.4      -0:48`nMilford       7.0         3.2      +0:05"; X=90; Y=470; Size=12; Style=$R }
    @{ Text='Tidal range and high water at four ports'; X=90; Y=610; Size=11; Style=$R }
    @{ Text='Hazards'; X=80; Y=678; Size=17; Style=$B }
    @{ Text="1.  The Runnel Stone, which breaks in any swell from the west.`n2.  The overfalls off the Lizard, worst on the ebb against a westerly.`n3.  The bar at Padstow, which is not to be attempted on the ebb."; X=90; Y=742; Size=13; Style=$R }
)

New-Page 2 @(
    @{ Text='Harbours of refuge'; X=80; Y=88; Size=17; Style=$B }
    @{ Text='Three harbours can be entered at any state of the tide in any weather a small vessel should be out in at all. They are given here from east to west, with the depth carried in over the sill.'; X=80; Y=158; Size=13; Style=$R }
    @{ Text="-  Falmouth, the deepest natural harbour on this coast.`n-  Newlyn, which dries in the inner basin but not at the wall.`n-  Milford Haven, entered in any wind and at any tide."; X=90; Y=272; Size=13; Style=$R }
    @{ Text='A vessel that cannot make one of these three should stand well off and wait for the tide rather than close the land in poor visibility.'; X=80; Y=418; Size=13; Style=$R }
)
