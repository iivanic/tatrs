# End-to-end agent flow: ready -> claim -> work -> close -> release, JSON only.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo build --workspace --quiet
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $bin = Join-Path $root 'target\debug\tatrs.exe'
    $scratch = Join-Path $root ('target\agent-flow-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Path $scratch | Out-Null

    try {
        & $bin init --cwd $scratch | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "init failed" }

        $body = "## Acceptance`n`n- [ ] the build passes"
        $task = & $bin new 'Ship the build' -t agent --body $body --json --cwd $scratch | ConvertFrom-Json
        if (-not $task.id) { throw "task creation produced no id" }

        $ready = & $bin ready --json --cwd $scratch | ConvertFrom-Json
        if ($ready.ready.Count -ne 1) { throw "expected 1 ready task, got $($ready.ready.Count)" }

        $claim = & $bin claim $task.id --owner agent-3 --ttl 2h --run flow-1 --json --cwd $scratch | ConvertFrom-Json
        if (-not $claim.claimed) { throw "claim did not report success" }
        if (-not $claim.token) { throw "claim produced no token" }

        $blocked = & $bin ready --json --cwd $scratch | ConvertFrom-Json
        if ($blocked.ready.Count -ne 0) { throw "claimed task is still listed as ready" }

        $closed = & $bin ls -u close $task.id --json --cwd $scratch | ConvertFrom-Json
        if ($closed.updated -ne 1) { throw "close did not update the task" }

        & $bin release $task.id --token $claim.token --json --cwd $scratch | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "release failed" }

        & $bin check --strict --cwd $scratch | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "check --strict failed after the flow" }

        Write-Host "agent flow OK: $($task.id)"
    }
    finally {
        Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
    }
}
finally {
    Pop-Location
}
