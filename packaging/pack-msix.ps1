# Packs commandui-desktop.exe into an unsigned MSIX for the existing
# Partner Center product. Partner Center signs the upload. This script
# does not sign, and it refuses a package whose identity drifted.
#Requires -Version 7
param(
    [string]$Exe,
    [string]$Out
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path $PSScriptRoot -Parent
$Template = Join-Path $PSScriptRoot 'msix'
$Manifest = Join-Path $Template 'AppxManifest.xml'

$ExpectedName = 'mcp-tool-shop.CommandUI'
$ExpectedPublisher = 'CN=5305D976-6952-4F00-9C21-3A5DB090359F'
$ExpectedArch = 'x64'
$ExpectedExe = 'commandui-desktop.exe'
$ExpectedEntry = 'Windows.FullTrustApplication'
$ExpectedDisplayName = 'CommandUI'
$ExpectedPublisherDisplay = 'mcp-tool-shop'
$VersionFloor = [version]'1.0.1.0'

function Find-MakeAppx {
    $roots = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'),
        (Join-Path $env:ProgramFiles 'Windows Kits\10\bin')
    )
    $hits = foreach ($root in $roots) {
        if (Test-Path $root) {
            Get-ChildItem $root -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match '\\x64\\makeappx.exe$' }
        }
    }
    $best = $hits | Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $best) {
        throw 'makeappx.exe (x64) was not found. Install the Windows 10 SDK.'
    }
    return $best.FullName
}

function Read-Identity([string]$Path) {
    [xml]$doc = Get-Content -Path $Path -Raw
    $ns = New-Object System.Xml.XmlNamespaceManager($doc.NameTable)
    $ns.AddNamespace('m', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10')
    $identity = $doc.SelectSingleNode('/m:Package/m:Identity', $ns)
    $props = $doc.SelectSingleNode('/m:Package/m:Properties', $ns)
    $app = $doc.SelectSingleNode('/m:Package/m:Applications/m:Application', $ns)
    if (-not $identity -or -not $props -or -not $app) {
        throw "Manifest is missing Identity, Properties, or Application: $Path"
    }
    return [pscustomobject]@{
        Name                 = [string]$identity.Name
        Publisher            = [string]$identity.Publisher
        Version              = [string]$identity.Version
        Arch                 = [string]$identity.ProcessorArchitecture
        Executable           = [string]$app.Executable
        EntryPoint           = [string]$app.EntryPoint
        DisplayName          = [string]$props.DisplayName
        PublisherDisplayName = [string]$props.PublisherDisplayName
    }
}

function Assert-Identity($id, [string]$Where) {
    $problems = @()
    if ($id.Name -ne $ExpectedName) { $problems += "name is '$($id.Name)'" }
    if ($id.Publisher -ne $ExpectedPublisher) { $problems += 'publisher does not match the Store product' }
    if ($id.Arch -ne $ExpectedArch) { $problems += "architecture is '$($id.Arch)'" }
    if ($id.Executable -ne $ExpectedExe) { $problems += "executable is '$($id.Executable)'" }
    if ($id.EntryPoint -ne $ExpectedEntry) { $problems += "entry point is '$($id.EntryPoint)'" }
    if ($id.DisplayName -ne $ExpectedDisplayName) { $problems += "display name is '$($id.DisplayName)'" }
    if ($id.PublisherDisplayName -ne $ExpectedPublisherDisplay) { $problems += "publisher display name is '$($id.PublisherDisplayName)'" }
    $parts = $id.Version.Split('.')
    if ($parts.Count -ne 4) {
        $problems += "version '$($id.Version)' is not four parts"
    } else {
        $parsed = [version]$id.Version
        if ($parsed -le $VersionFloor) {
            $problems += "version $($id.Version) is not above 1.0.1.0"
        }
    }
    if ($problems.Count -gt 0) {
        throw ("Identity check failed ({0}): " -f $Where) + ($problems -join '; ')
    }
}

function Find-Exe {
    $candidates = @(
        (Join-Path $RepoRoot 'target\release\commandui-desktop.exe'),
        (Join-Path $RepoRoot 'apps\desktop\src-tauri\target\release\commandui-desktop.exe')
    )
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) { return $candidate }
    }
    throw "commandui-desktop.exe was not found. Looked in:`n$($candidates -join "`n")"
}

if (-not (Test-Path $Manifest)) {
    throw "Missing $Manifest"
}

$sourceId = Read-Identity $Manifest
Assert-Identity $sourceId 'packaging/msix/AppxManifest.xml'

if (-not $Exe) { $Exe = Find-Exe }
if (-not (Test-Path $Exe)) { throw "Executable not found: $Exe" }
$exeItem = Get-Item $Exe
if ($exeItem.Length -lt 1MB) {
    throw "Executable is $($exeItem.Length) bytes. A Tauri release binary is larger than that."
}

$version = $sourceId.Version
if (-not $Out) {
    $releaseDir = Join-Path $RepoRoot 'release'
    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
    $Out = Join-Path $releaseDir "CommandUI_${version}_x64.msix"
}

$makeappx = Find-MakeAppx
$stage = Join-Path $env:TEMP ("commandui-msix-stage-" + [guid]::NewGuid().ToString('n'))
$check = Join-Path $env:TEMP ("commandui-msix-check-" + [guid]::NewGuid().ToString('n'))

try {
    New-Item -ItemType Directory -Path $stage | Out-Null
    Copy-Item -Path (Join-Path $Template '*') -Destination $stage -Recurse -Force
    Copy-Item -Path $Exe -Destination (Join-Path $stage $ExpectedExe) -Force

    & $makeappx pack /o /d $stage /p $Out
    if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed with exit $LASTEXITCODE" }

    $listing = & tar -tf $Out
    if ($listing -match 'AppxSignature\.p7x') {
        throw 'Package contains AppxSignature.p7x. The Store upload must be unsigned.'
    }
    if ($listing -notcontains $ExpectedExe) {
        throw "Packed archive is missing $ExpectedExe"
    }

    New-Item -ItemType Directory -Path $check | Out-Null
    & tar -xf $Out -C $check AppxManifest.xml
    $packedId = Read-Identity (Join-Path $check 'AppxManifest.xml')
    Assert-Identity $packedId 'packed AppxManifest.xml'
    if ($packedId.Version -ne $version) {
        throw "Packed version $($packedId.Version) does not match source $version"
    }
}
finally {
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    if (Test-Path $check) { Remove-Item -Recurse -Force $check }
}

$scanner = Join-Path $env:USERPROFILE '.grok\bin\identity-scan.py'
if (Test-Path $scanner) {
    & python $scanner $Out
    if ($LASTEXITCODE -ne 0) {
        Remove-Item -Force $Out
        throw 'Identity scan HIT. The package was deleted and must not be uploaded.'
    }
} else {
    # The scan is a local pre-upload step: the scanner lives only on the
    # author's rig, so CI cannot run it. Say so loudly instead of passing quietly.
    if ($env:GITHUB_ACTIONS -eq 'true') {
        Write-Output '::notice title=MSIX identity scan not run::identity-scan.py is not on this runner. The identity scan is a local pre-upload step; this CI-built package was NOT scanned. Run it on the author rig before publishing the artifact.'
    }
    Write-Output 'identity-scan skipped (scanner absent; local pre-upload step, package NOT scanned)'
}

Write-Output "RESULT PASS"
Write-Output "package $Out"
Write-Output "name $($sourceId.Name)"
Write-Output "version $($sourceId.Version)"
Write-Output "arch $($sourceId.Arch)"
Write-Output "executable $($sourceId.Executable)"
Write-Output "signed no"
