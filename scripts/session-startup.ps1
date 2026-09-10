<#
.SYNOPSIS
  Idempotent per-session bootstrap. Starts the 30s liveness watch that keeps the local app alive.

.DESCRIPTION
  Called automatically by the agent at session start (see the "Session startup" directive in
  .github/copilot-instructions.md). Warns if dev secrets are missing, then starts the detached liveness watch
  (scripts/run-app.ps1 Watch) unless one is already running (single-instance via
  scripts/.liveness-watch.pid). The watch ensures the app comes up and keeps it alive: every 30s it
  probes the `/api/health` liveness endpoint and, when down, restarts only if the coder state is
  `ready`, otherwise it polls for `ready`. For a manual clean restart use the run-app skill (./scripts/run-app.ps1 Restart).

  Lifecycle safety valve: on a fresh boot (this bootstrap finds no live watch and is about to start
  one), any leftover `building`/`broken` signal from a prior, now-gone session is reset to `ready`
  via scripts/set-dev-state.ps1 before the watch starts. This runs ONLY on the fresh-boot path — if a
  watch is already live it no-ops and dev-state is left untouched. There is no time-based coercion.

  Conditional Cosmos preflight: on the fresh-boot path, before starting the watch, it runs
  scripts/cosmos-preflight.ps1, which waits for the Cosmos DB Emulator ONLY when local dev is opted
  into Cosmos (TASKLIST_PERSISTENCE=cosmos). On the default in-memory loop it is a no-op (no probe,
  no wait); the preflight is advisory and never blocks the watch from starting.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$secretsFile = Join-Path $PSScriptRoot 'dev-secrets.local.ps1'
$runApp = Join-Path $PSScriptRoot 'run-app.ps1'
$pidFile = Join-Path $PSScriptRoot '.liveness-watch.pid'
$stateFile = Join-Path $PSScriptRoot '.dev-state.json'
$setState = Join-Path $PSScriptRoot 'set-dev-state.ps1'
$cosmosPreflight = Join-Path $PSScriptRoot 'cosmos-preflight.ps1'

if (-not (Test-Path $secretsFile)) {
    Write-Warning "scripts/dev-secrets.local.ps1 missing — copy dev-secrets.template.ps1 and fill it in. The watch will start but the app can't fully start until secrets exist."
}

function Test-RecordedScriptRunning([string]$PidPath, [string]$ScriptPath) {
    if (-not (Test-Path $PidPath)) { return $false }
    $existing = (Get-Content $PidPath -Raw).Trim()
    if (-not $existing -or $existing -notmatch '^\d+$') { return $false }
    $process = Get-CimInstance Win32_Process -Filter "ProcessId = $existing" -ErrorAction SilentlyContinue
    if (-not $process -or -not $process.CommandLine) { return $false }
    $leaf = Split-Path -Leaf $ScriptPath
    return $process.CommandLine.IndexOf($leaf, [StringComparison]::OrdinalIgnoreCase) -ge 0
}

if (Test-Path $pidFile) {
    $existing = (Get-Content $pidFile -Raw).Trim()
    if ($existing -and (Test-RecordedScriptRunning $pidFile $runApp)) {
        Write-Host "liveness watch already running (pid=$existing); nothing to do."
        return
    }
}

# Fresh session boot: we reach here only when the guard above found no live watch and we are about
# to start one. The prior session that set any `building`/`broken` signal is gone, so clearing it to
# `ready` is deterministic and cannot race a live edit. Scoped to fresh boot only — if a watch was
# already live the guard returned above and dev-state is never touched. No time-based coercion.
if (Test-Path $stateFile) {
    try {
        $current = (Get-Content $stateFile -Raw | ConvertFrom-Json).state
        if ($current -in @('building', 'broken')) {
            & $setState ready -Note "session boot: cleared stale '$current' from a prior session"
        }
    }
    catch { Write-Warning "session-startup: could not read scripts/.dev-state.json to clear a stale signal: $($_.Exception.Message)" }
}

# Conditional Cosmos preflight (fresh-boot path only): wait for the Cosmos DB Emulator ONLY when
# local dev is opted into Cosmos persistence (TASKLIST_PERSISTENCE=cosmos). On the default
# in-memory loop this is a no-op — no probe, no wait — so the fast startup is preserved exactly. It
# is advisory (never blocks): on timeout it prints an actionable message and the watch still starts.
if (Test-Path $cosmosPreflight) {
    try { & $cosmosPreflight }
    catch { Write-Warning "session-startup: Cosmos preflight error (ignored): $($_.Exception.Message)" }
}

Start-Process pwsh -ArgumentList '-NoProfile', '-File', $runApp, 'Watch' -WindowStyle Hidden
Write-Host "liveness watch started. Log: scripts/liveness-watch.log"
