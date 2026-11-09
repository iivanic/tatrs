# Package one release binary as a zip plus a SHA-256 checksum.
# Runs on every GitHub runner (pwsh is preinstalled), so the workflow needs no
# platform-specific packaging branch.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Binary,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$OutDir
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Binary)) {
    throw "binary not found: $Binary"
}

$version = $Version.TrimStart('v')
$name = "tatrs-$version-$Target"
$out = Join-Path $OutDir $name
$stage = Join-Path ([System.IO.Path]::GetTempPath()) "$name-$([guid]::NewGuid().ToString('N').Substring(0, 8))"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
New-Item -ItemType Directory -Force -Path $stage | Out-Null

try {
    Copy-Item $Binary (Join-Path $stage (Split-Path $Binary -Leaf))
    foreach ($extra in @('README.md', 'LICENSE')) {
        if (Test-Path $extra) {
            Copy-Item $extra (Join-Path $stage $extra)
        }
    }

    $zip = Join-Path $OutDir "$name.zip"
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force

    $hash = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $name.zip" | Set-Content -Path "$zip.sha256" -NoNewline

    Write-Host "packaged $zip"
}
finally {
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
}
