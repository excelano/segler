# Remove what install.ps1 put in place, and tell the shell it is gone.
#
# The whole of it, because a file association that outlives its executable is
# worse than none: Explorer keeps drawing the icon and offering the type, and
# double-clicking fails with a message about a missing file rather than the
# dialog that would have let a person pick something else.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

[CmdletBinding()]
param(
    [string] $Prefix = (Join-Path $env:LOCALAPPDATA 'Programs\Segler'),
    # Leave the installed executables and icons where they are.
    [switch] $KeepFiles
)

$ErrorActionPreference = 'Stop'

# The same table install.ps1 writes from, and it has to stay the same table: a
# type added there and not here is exactly the dead association this script
# exists to prevent.
$Types = @(
    @{ Extension = '.dclx'; ContentType = 'application/vnd.doclang.archive+zip';  ProgId = 'Excelano.Segler.Archive'  },
    @{ Extension = '.dclg'; ContentType = 'application/vnd.doclang.document+xml'; ProgId = 'Excelano.Segler.Document' }
)

$exeName = 'segler-desktop.exe'
$cliName = 'segler.exe'
$icons = @('segler.ico', 'dclx.ico', 'dclg.ico')

# The .NET API for the same reason install.ps1 uses it: PowerShell's registry
# provider reads the forward slash in a media type as a path separator, so it
# would look for the wrong key here and leave the right one behind.
function Test-OurKey {
    param([string] $Path)
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Path, $false)
    if (-not $key) { return $false }
    $key.Close()
    return $true
}

# A key that is not there is nothing to do; a key that is there and will not go
# is a failure, and catching every exception cannot tell the two apart. Both are
# read back, so the only thing passed over is the absence.
function Remove-Key {
    param([string] $Path)
    if (-not (Test-OurKey $Path)) { return }
    [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($Path, $false)
    if (Test-OurKey $Path) { throw "uninstall.ps1: HKCU\$Path is still there after being deleted" }
}

# The same, for a key that cannot be opened for writing. Explorer writes a
# *Deny SetValue* rule on UserChoice so no application can quietly take an
# extension over, and `DeleteSubKeyTree` opens the key itself for writing before
# deleting it, so it fails against the rule — and .NET reads that failure as the
# key being missing and returns quietly. Deleting the name from the parent needs
# DELETE on the child and nothing else, which the rule beside the deny allows.
function Remove-Subkey {
    param([string] $Parent, [string] $Name)
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Parent, $true)
    if (-not $key) { return }
    try { $key.DeleteSubKey($Name, $false) } finally { $key.Close() }
    if (Test-OurKey "$Parent\$Name") {
        throw "uninstall.ps1: HKCU\$Parent\$Name is still there after being deleted"
    }
}

# A value removed only if it holds what install.ps1 wrote, so that another
# application's registration on the same extension is not touched.
function Remove-OurValue {
    param([string] $Path, [string] $Name, [string] $Ours)
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Path, $true)
    if (-not $key) { return }
    try {
        $held = $key.GetValue($Name, $null)
        if ($null -ne $held -and ($Ours -eq '' -or $held -eq $Ours)) {
            $key.DeleteValue($Name, $false)
        }
    } finally {
        $key.Close()
    }
}

$classes = 'Software\Classes'

foreach ($type in $Types) {
    Remove-Key "$classes\$($type.ProgId)"
    # The extension's own values rather than the whole key. `OpenWithProgids`
    # belongs to the extension and to every application that has ever offered to
    # open one, so removing the tree takes somebody else's offer with it.
    Remove-OurValue "$classes\$($type.Extension)\OpenWithProgids" $type.ProgId ''
    Remove-OurValue "$classes\$($type.Extension)" '' $type.ProgId
    Remove-OurValue "$classes\$($type.Extension)" 'Content Type' $type.ContentType
    Remove-Key "$classes\MIME\Database\Content Type\$($type.ContentType)"

    # The one that is easy to miss. Choosing "always open with" writes a
    # UserChoice here, and a UserChoice outranks everything removed above:
    # leaving it behind leaves the extension pointing at a ProgID that no longer
    # exists, which is the dead association this script exists to prevent.
    # Windows treats such a choice as no association at all rather than falling
    # back to the machine-wide one. Measured on slipcase-desktop, where
    # `README.md` beside this file records what each stale shape does to a
    # double-click.
    #
    # The UserChoice key by name through `Remove-Subkey`, and not the FileExts
    # tree above it: that tree holds other applications' entries, and the deny
    # rule on UserChoice defeats a tree delete silently. Removed only when it
    # names this application.
    $exts = "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\$($type.Extension)"
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("$exts\UserChoice", $false)
    if ($key) {
        $chosen = $key.GetValue('ProgId', $null)
        $key.Close()
        if ($chosen -eq $type.ProgId) { Remove-Subkey $exts 'UserChoice' }
    }
}

Remove-Key "$classes\Applications\$exeName"
Remove-Key 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Segler'

$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Segler.lnk'
if (Test-Path -LiteralPath $shortcut) { Remove-Item -LiteralPath $shortcut -Force -Confirm:$false }

if (-not $KeepFiles) {
    foreach ($name in (@($exeName, $cliName) + $icons)) {
        $path = Join-Path $Prefix $name
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force -Confirm:$false }
    }
    # Add/Remove Programs points at the copy inside the directory, so the usual
    # run is a script emptying the directory it is itself in, and a running
    # script cannot delete itself. Run from a checkout it is not that file, and
    # then the copy is an ordinary file that can go with the rest.
    #
    # Both branches were one line in slipcase-desktop until 2026-08-26, when a
    # run from the checkout left the copy behind and said it was the script now
    # running. That was untrue and it left a directory the script says it
    # removes, so the two cases are told apart rather than assumed to be one.
    $copy = Join-Path $Prefix 'uninstall.ps1'
    $self = $MyInvocation.MyCommand.Path
    if (Test-Path -LiteralPath $copy) {
        $same = $self -and
            ([System.IO.Path]::GetFullPath($self) -ieq [System.IO.Path]::GetFullPath($copy))
        if ($same) {
            Write-Output "left ${copy} behind: it is the script now running"
        } else {
            Remove-Item -LiteralPath $copy -Force -Confirm:$false
        }
    }
    # And the directory, where emptying it emptied it. Left alone if anything
    # else is in there, because this script installed none of it.
    if ((Test-Path -LiteralPath $Prefix) -and
        -not (Get-ChildItem -LiteralPath $Prefix -Force)) {
        Remove-Item -LiteralPath $Prefix -Force -Confirm:$false
    }
}

Add-Type -Namespace SeglerUninstall -Name Shell -MemberDefinition @'
[DllImport("shell32.dll", CharSet=CharSet.Unicode)]
public static extern void SHChangeNotify(int eventId, uint flags, System.IntPtr item1, System.IntPtr item2);
'@
[SeglerUninstall.Shell]::SHChangeNotify(0x08000000, 0, [System.IntPtr]::Zero, [System.IntPtr]::Zero)

Write-Output "removed both DocLang associations, the Start menu entry, and the uninstall entry"
