# Compatibility check against the upstream tatr corpus in reference/tatr.
# Read-only: it never writes into the reference checkout.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $reference = Join-Path $root 'reference\tatr'
    if (-not (Test-Path $reference)) {
        Write-Host 'compat: reference/tatr is absent, skipping'
        exit 0
    }

    cargo build --workspace --quiet
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $bin = Join-Path $root 'target\debug\tatrs.exe'
    $expected = (Get-ChildItem (Join-Path $reference 'tasks') -Directory).Count

    $listed = & $bin ls --all --json --cwd $reference | ConvertFrom-Json
    if ($listed.tasks.Count -ne $expected) {
        throw "parsed $($listed.tasks.Count) tasks, expected $expected"
    }

    $check = & $bin check --json --cwd $reference | ConvertFrom-Json
    if ($check.errors -ne 0) {
        $codes = $check.diagnostics | Where-Object { $_.severity -eq 'error' } | ForEach-Object { $_.code }
        throw "upstream corpus produced $($check.errors) error(s): $($codes -join ', ')"
    }

    Write-Host "compat OK: $($listed.tasks.Count) tasks, $($check.warnings) warning(s)"
}
finally {
    Pop-Location
}
