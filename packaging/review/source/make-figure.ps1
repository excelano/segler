# Draw the figure the document's <picture> points at, `assets/figure.png`.
#
# A tide curve, chosen because it has a shape recognisable at any zoom: the
# picture pane scales an asset to the box the <location> elements give it, and
# a photograph or a block of colour proves nothing about whether that worked.
# The axis labels the document repeats as inner <text> - "0h", "12h", "24h",
# and the four heights - are drawn here at the same places, so the two can be
# read against each other.
#
# Needs Windows: `System.Drawing` and the Georgia font. See the note in
# `make-pages.ps1` about why the output is committed rather than regenerated.
#
#   powershell -ExecutionPolicy Bypass -File packaging\review\source\make-figure.ps1
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$out = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'assets'
New-Item -ItemType Directory -Force -Path $out | Out-Null

$W = 620; $H = 360
$bmp = New-Object System.Drawing.Bitmap($W, $H)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.TextRenderingHint = 'AntiAliasGridFit'
$g.Clear([System.Drawing.Color]::FromArgb(252, 251, 247))

$axis = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(60, 62, 68)), 2
$grid = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(214, 212, 205)), 1
$curve = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(74, 111, 165)), 3
$ink = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(40, 42, 46))
$font = New-Object System.Drawing.Font('Georgia', 11)
$small = New-Object System.Drawing.Font('Georgia', 9)

$L = 70; $R = $W - 30; $T = 40; $B = $H - 50

for ($i = 0; $i -le 4; $i++) {
    $y = $T + ($B - $T) * $i / 4
    $g.DrawLine($grid, $L, $y, $R, $y)
    $g.DrawString((4 - $i).ToString(), $small, $ink, ($L - 22), ($y - 8))
}
$g.DrawLine($axis, $L, $T, $L, $B)
$g.DrawLine($axis, $L, $B, $R, $B)

# Two tidal cycles.
$pts = New-Object System.Collections.Generic.List[System.Drawing.PointF]
for ($x = 0; $x -le 100; $x++) {
    $t = $x / 100.0
    $v = 2.0 + 1.9 * [Math]::Sin($t * 4 * [Math]::PI - 1.2)
    $px = $L + ($R - $L) * $t
    $py = $B - ($B - $T) * ($v / 4.0)
    $pts.Add((New-Object System.Drawing.PointF($px, $py)))
}
$g.DrawCurve($curve, $pts.ToArray())

$g.DrawString('Tidal height at Falmouth, metres', $font, $ink, $L, 12)
$g.DrawString('0h', $small, $ink, $L, ($B + 8))
$g.DrawString('12h', $small, $ink, ($L + ($R - $L) / 2 - 10), ($B + 8))
$g.DrawString('24h', $small, $ink, ($R - 20), ($B + 8))

$axis.Dispose(); $grid.Dispose(); $curve.Dispose(); $ink.Dispose()
$font.Dispose(); $small.Dispose(); $g.Dispose()
$path = Join-Path $out 'figure.png'
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Host "wrote $path"
