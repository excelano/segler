# Pack `sailing-directions.dclx` from the sources in this directory.
#
# A `.dclx` is an OPC package: a Zip carrying `[Content_Types].xml`, an
# `_rels/.rels` naming the document part, the document itself, and whatever
# page images and assets it points at. The two XML parts are written here
# rather than kept as files, because they are four lines each and a package
# whose content types live in a separate file is a package that can be
# assembled with one of them missing.
#
# The entry order matters to nothing in the specification and is kept anyway:
# the manifest parts first, then the document, then its images, which is the
# order a reader needs them in.
#
#   powershell -ExecutionPolicy Bypass -File packaging\review\source\pack.ps1
#   powershell -ExecutionPolicy Bypass -File packaging\review\source\pack.ps1 -Out C:\tmp\check.dclx
#
# The default writes over `packaging/review/sailing-directions.dclx`. Deflate
# is not reproducible across implementations, so the committed archive and a
# fresh pack of the same sources are equal member by member and not byte for
# byte; `-Out` is there to compare without overwriting.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
param(
    [string]$Out
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Out) { $Out = Join-Path (Split-Path -Parent $here) 'sailing-directions.dclx' }

$types = @'
<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/document.xml" ContentType="application/vnd.doclang.document+xml"/>
</Types>

'@
$rels = @'
<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://doclang.ai/ns/package/2026/relationships/document" Target="document.xml"/>
</Relationships>

'@

# UTF-8 with no byte order mark, and LF, whatever this file was checked out as.
$utf8 = New-Object System.Text.UTF8Encoding $false
function Get-PartBytes {
    param([string]$Text)
    return $utf8.GetBytes(($Text -replace "`r`n", "`n"))
}

$parts = [ordered]@{
    '[Content_Types].xml' = Get-PartBytes $types
    '_rels/.rels'         = Get-PartBytes $rels
    'document.xml'        = [System.IO.File]::ReadAllBytes((Join-Path $here 'sailing-directions.dclg'))
    'pages/1.png'         = [System.IO.File]::ReadAllBytes((Join-Path $here 'pages\1.png'))
    'pages/2.png'         = [System.IO.File]::ReadAllBytes((Join-Path $here 'pages\2.png'))
    'assets/figure.png'   = [System.IO.File]::ReadAllBytes((Join-Path $here 'assets\figure.png'))
}

if (Test-Path $Out) { Remove-Item $Out }
$zip = [System.IO.Compression.ZipFile]::Open($Out, 'Create')
try {
    foreach ($name in $parts.Keys) {
        $entry = $zip.CreateEntry($name, [System.IO.Compression.CompressionLevel]::Optimal)
        $stream = $entry.Open()
        try { $stream.Write($parts[$name], 0, $parts[$name].Length) } finally { $stream.Dispose() }
        Write-Host ("  {0,-20} {1,7} bytes" -f $name, $parts[$name].Length)
    }
} finally {
    $zip.Dispose()
}
Write-Host "wrote $Out - $((Get-Item $Out).Length) bytes"
