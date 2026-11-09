# End-to-end smoke test for tatrs. Runs against a scratch repository inside
# target/ because some sandboxes deny writes to the system temp directory.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo build --workspace --quiet
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $bin = Join-Path $root 'target\debug\tatrs.exe'
    if (-not (Test-Path $bin)) { throw "tatrs binary not found at $bin" }

    $scratch = Join-Path $root ('target\smoke-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Path $scratch | Out-Null

    try {
        & $bin init --cwd $scratch | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "init failed" }

        $created = & $bin new 'Smoke task' -t smoke,agent --acceptance --json --cwd $scratch | ConvertFrom-Json
        if ($created.schema -ne 1) { throw "unexpected schema: $($created.schema)" }
        if ($created.status -ne 'OPEN') { throw "unexpected status: $($created.status)" }
        if ($created.tags.Count -ne 2) { throw "unexpected tags: $($created.tags -join ',')" }
        if (-not (Test-Path (Join-Path $scratch $created.path))) { throw "TASK.md was not created" }

        $listed = & $bin ls --json --cwd $scratch | ConvertFrom-Json
        if ($listed.tasks.Count -ne 1) { throw "ls returned $($listed.tasks.Count) tasks" }

        $shown = & $bin show $created.id --json --cwd $scratch | ConvertFrom-Json
        if ($shown.body -notmatch 'Acceptance') { throw "acceptance section missing" }

        & $bin check --strict --cwd $scratch | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "check --strict failed on a fresh task" }

        & $bin new 'blocked' --read-only --cwd $scratch *> $null
        if ($LASTEXITCODE -ne 2) { throw "read-only new exited $LASTEXITCODE, expected 2" }

        Write-Host "smoke OK: $($created.id)"
    }
    finally {
        Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
    }
}
finally {
    Pop-Location
}
