<#
.SYNOPSIS
    Refuse a binary that imports a DLL Windows does not ship.

.DESCRIPTION
    Slipcase 0.1.1 failed Microsoft Store certification on 2026-08-29 under
    policy 10.2.4.1. The package installed on the tester's clean machine and
    then would not start: *The code execution cannot proceed because
    VCRUNTIME140.dll was not found.* That DLL ships in the Visual C++
    Redistributable, not in Windows. `.cargo/config.toml` in this repository
    carries the fix - `+crt-static` for this target - and this script is the
    guard on it.

    Nothing this project runs could have caught it there. The tests pass, the
    conformance corpus passes, the certification kit passed, and the
    application starts - on a machine with Visual Studio installed, which is
    every machine any of the three platforms has ever built on. The defect is
    invisible from inside the toolchain that causes it, so the check has to be
    about the artefact rather than about whether it runs here.

    This is the Windows analogue of the `ldd` line `linux.yml` uses to check
    what the Linux binary links, and it exists for the same reason: the rule
    means the outcome, so check the outcome.

    It parses the PE import table itself rather than shelling out to dumpbin,
    because dumpbin comes with Visual C++ and a check that needs the toolchain
    is a check that cannot run where the toolchain is absent.

.PARAMETER Binary
    The executable to read. Defaults to the release build of segler-desktop.
#>
[CmdletBinding()]
param(
    [string] $Binary
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Refuse([string] $why) {
    Write-Host "check-imports: $why" -ForegroundColor Red
    exit 1
}

# Every DLL here has been confirmed present in C:\Windows\System32 on a stock
# Windows 10 or 11 install. The list is deliberately explicit: a dependency
# this project has never seen before should stop a build and be looked at by a
# person, which is the step that was missing when VCRUNTIME140.dll arrived.
#
# It is slipcase-desktop's list unchanged, which was worth checking rather than
# assuming: this application renders through wgpu where that one renders through
# glow, and the expectation was that Direct3D would add names here. It does not.
# The eighteen below are exactly what `segler-desktop.exe` imports, measured on
# 2026-09-04 against the release binary, and the two lists are the same because
# what a window imports is the window, not the renderer.
#
# **d3d12.dll is deliberately absent and this check does not cover it.** wgpu
# reaches Direct3D through `LoadLibrary` at run time rather than through the
# import table, so a missing graphics library would fail somewhere this script
# cannot see. That is not a gap worth closing here: d3d12.dll and dxgi.dll are
# Windows components, wgpu falls back when a backend will not load, and a check
# that tried to follow run-time loading would be guessing at names.
$InBox = @(
    'advapi32.dll', 'bcryptprimitives.dll', 'combase.dll', 'dwmapi.dll',
    'dxgi.dll', 'gdi32.dll', 'imm32.dll', 'kernel32.dll', 'ntdll.dll',
    'ole32.dll', 'oleaut32.dll', 'opengl32.dll', 'setupapi.dll',
    'shell32.dll', 'shlwapi.dll', 'user32.dll', 'uiautomationcore.dll',
    'uxtheme.dll'
)

# API set contracts are resolved by the loader from the schema inside Windows
# itself; there is no file to be missing. api-ms-win-crt-* is the Universal C
# Runtime, which is a Windows component from Windows 10 onward - it is the
# *Visual C++* runtime beside it that is not.
$InBoxPrefixes = @('api-ms-win-', 'ext-ms-win-')

if (-not $Binary) {
    $here = Split-Path -Parent $MyInvocation.MyCommand.Path
    $Binary = Join-Path $here '..\..\target\release\segler-desktop.exe'
    # `[build] target-dir` moves the target directory and no environment
    # variable then says so, which is why the packaging scripts ask cargo.
    $meta = cargo metadata --format-version 1 --no-deps 2>$null | ConvertFrom-Json
    if ($meta) { $Binary = Join-Path $meta.target_directory 'release\segler-desktop.exe' }
}

if (-not (Test-Path $Binary)) { Refuse "no binary at $Binary - run 'cargo build --release' first" }
$bytes = [System.IO.File]::ReadAllBytes($Binary)

function U16([int] $at) { return [System.BitConverter]::ToUInt16($bytes, $at) }
function U32([int] $at) { return [System.BitConverter]::ToUInt32($bytes, $at) }

if ((U16 0) -ne 0x5A4D) { Refuse "$Binary does not start with MZ" }
$pe = [int](U32 0x3C)
if ((U32 $pe) -ne 0x00004550) { Refuse "$Binary has no PE signature at e_lfanew" }

$sizeOfOptional = [int](U16 ($pe + 20))
$opt = $pe + 24
$magic = U16 $opt
if ($magic -ne 0x20B) { Refuse ("$Binary is not PE32+ (magic 0x{0:X}) - this build targets x64" -f $magic) }

# PE32+ data directories begin 112 bytes into the optional header; entry 1 is
# the import table and entry 13 is the delay-load table. Both are walked: a
# delay-loaded DLL is just as absent on the machine that lacks it, it merely
# fails later.
$importRva = U32 ($opt + 112 + (1 * 8))
$delayRva  = U32 ($opt + 112 + (13 * 8))

$sections = @()
$sectionTable = $pe + 24 + $sizeOfOptional
for ($i = 0; $i -lt [int](U16 ($pe + 6)); $i++) {
    $s = $sectionTable + ($i * 40)
    $sections += [pscustomobject]@{
        Virtual = U32 ($s + 12)
        Size    = [Math]::Max((U32 ($s + 8)), (U32 ($s + 16)))
        Raw     = U32 ($s + 20)
    }
}

function Offset([uint32] $rva) {
    foreach ($s in $sections) {
        if ($rva -ge $s.Virtual -and $rva -lt ($s.Virtual + $s.Size)) {
            return [int]($rva - $s.Virtual + $s.Raw)
        }
    }
    Refuse ("RVA 0x{0:X} falls in no section" -f $rva)
}

function NameAt([uint32] $rva) {
    $at = Offset $rva
    $end = $at
    while ($bytes[$end] -ne 0) { $end++ }
    return [System.Text.Encoding]::ASCII.GetString($bytes, $at, $end - $at)
}

$imports = @()
if ($importRva -ne 0) {
    $at = Offset $importRva
    while ((U32 ($at + 12)) -ne 0) {      # the Name RVA; a zero descriptor ends the table
        $imports += NameAt (U32 ($at + 12))
        $at += 20
    }
}
if ($delayRva -ne 0) {
    $at = Offset $delayRva
    while ((U32 ($at + 4)) -ne 0) {       # DllNameRVA
        $imports += NameAt (U32 ($at + 4))
        $at += 32
    }
}

if ($imports.Count -eq 0) { Refuse "$Binary imports nothing, which cannot be right - the parse is wrong" }

$unknown = @()
foreach ($dll in $imports) {
    $lower = $dll.ToLowerInvariant()
    $ok = $InBox -contains $lower
    if (-not $ok) {
        foreach ($p in $InBoxPrefixes) { if ($lower.StartsWith($p)) { $ok = $true } }
    }
    if (-not $ok -and ($unknown -notcontains $dll)) { $unknown += $dll }
}

$distinct = $imports | Sort-Object -Unique
Write-Host "check-imports: $Binary"
Write-Host "  $($distinct.Count) distinct imports, $($unknown.Count) not known to ship with Windows"

if ($unknown.Count -gt 0) {
    foreach ($dll in $unknown) { Write-Host "  UNKNOWN  $dll" -ForegroundColor Red }
    Write-Host ''
    Write-Host 'A DLL that is not part of Windows has to be on the machine before' -ForegroundColor Yellow
    Write-Host 'Segler will start, and a Store tester will have a clean machine.' -ForegroundColor Yellow
    Write-Host 'If it is genuinely in-box, add it to $InBox above and say how that' -ForegroundColor Yellow
    Write-Host 'was confirmed. If it is not, remove the dependency.' -ForegroundColor Yellow
    exit 1
}

Write-Host '  every import ships with Windows' -ForegroundColor Green
exit 0
