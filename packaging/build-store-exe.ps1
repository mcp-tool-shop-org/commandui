# Release build of commandui-desktop.exe for the Store package.
# Remaps the user-profile prefix out of the binary. A release exe built
# without that remap embeds the Cargo registry path. pack-msix.ps1
# deletes the package when the identity scan hits.
#Requires -Version 7
$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path $PSScriptRoot -Parent
$userProfile = $env:USERPROFILE
if (-not $userProfile) { throw 'USERPROFILE is not set' }

$prefixes = @(
    ('\\?\' + $userProfile),
    $userProfile,
    ($userProfile -replace '\\', '/')
)
if ($env:CARGO_HOME) {
    $prefixes += $env:CARGO_HOME
    $prefixes += ($env:CARGO_HOME -replace '\\', '/')
}
$env:RUSTFLAGS = ($prefixes | ForEach-Object { "--remap-path-prefix=$_=/src" }) -join ' '

Set-Location $RepoRoot
pnpm --filter @commandui/desktop exec tauri build --no-bundle
if ($LASTEXITCODE -ne 0) { throw "tauri build failed with exit $LASTEXITCODE" }

Write-Output 'RESULT PASS'
Write-Output 'executable target\release\commandui-desktop.exe'
Write-Output 'next ./packaging/pack-msix.ps1'
