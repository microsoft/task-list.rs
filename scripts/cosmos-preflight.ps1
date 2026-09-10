<#
.SYNOPSIS
  Conditional local-dev preflight: wait for the Cosmos DB Emulator ONLY when the local app is
  opted into Cosmos persistence. A no-op on the default in-memory loop.

.DESCRIPTION
  Resolves the local app's persistence mode exactly the way scripts/dev.ps1 does — dot-sourcing
  scripts/dev-secrets.local.ps1 and then reading `$env:TASKLIST_PERSISTENCE` (case-insensitive,
  `cosmos` => Cosmos, anything else => in-memory; mirrors infrastructure `Persistence::parse`).
  The resolve runs in an isolated snapshot/restore so this script leaves the caller's environment
  untouched (dot-sourcing sets process-global env vars; we only READ them).

    - Mode != cosmos (default): returns immediately. No probe, no wait — the fast in-memory
      startup is preserved exactly.
    - Mode == cosmos: bounded-waits for the native Windows Cosmos DB Emulator gateway (default
      https://localhost:8081, self-signed cert). On timeout it prints an ACTIONABLE message and
      returns — it never hangs, and never hard-fails: local Cosmos is opt-in and this gate is
      advisory (always exits 0), so the app + liveness watch still start.

.NOTES
  Scripts/docs only — no app code. Does NOT touch the /api/health contract, scripts/.dev-state.json,
  set-dev-state.ps1, or the fresh-boot lifecycle reset. Invoked by scripts/session-startup.ps1 before
  the liveness watch starts; also runnable directly to observe the branch logic.

.EXAMPLE
  ./scripts/cosmos-preflight.ps1
.EXAMPLE
  # Observe the Cosmos wait/timeout branch without a real secrets file:
  $env:TASKLIST_PERSISTENCE='cosmos'; ./scripts/cosmos-preflight.ps1 -TimeoutSeconds 6 -SecretsFile 'nul'
#>
[CmdletBinding()]
param(
    [int]$TimeoutSeconds = 60,
    [int]$IntervalSeconds = 3,
    [string]$SecretsFile = (Join-Path $PSScriptRoot 'dev-secrets.local.ps1')
)

$ErrorActionPreference = 'Stop'
$DefaultEndpoint = 'https://localhost:8081'

# Resolve persistence mode + Cosmos endpoint the way dev.ps1 does, without leaking env vars.
function Resolve-Persistence {
    param([string]$SecretsFile)

    $snapshot = @{}
    Get-ChildItem Env: | ForEach-Object { $snapshot[$_.Name] = $_.Value }

    $mode = ''
    $endpoint = ''
    try {
        if ($SecretsFile -and (Test-Path $SecretsFile)) { . $SecretsFile }
        $mode = "$env:TASKLIST_PERSISTENCE".Trim().ToLowerInvariant()
        $endpoint = "$env:COSMOS__ENDPOINT".Trim()
    }
    catch {
        Write-Warning "cosmos-preflight: could not resolve persistence mode ($($_.Exception.Message)); assuming in-memory."
        $mode = ''
    }
    finally {
        # Restore the environment to its pre-dot-source state — remove additions, reset changes.
        foreach ($name in @(Get-ChildItem Env: | ForEach-Object Name)) {
            if (-not $snapshot.ContainsKey($name)) { Remove-Item "Env:$name" -ErrorAction SilentlyContinue }
        }
        foreach ($name in $snapshot.Keys) {
            if ([Environment]::GetEnvironmentVariable($name) -ne $snapshot[$name]) {
                Set-Item "Env:$name" $snapshot[$name] -ErrorAction SilentlyContinue
            }
        }
    }

    if (-not $endpoint) { $endpoint = $DefaultEndpoint }
    return [pscustomobject]@{
        Mode     = if ($mode -eq 'cosmos') { 'cosmos' } else { 'memory' }
        Endpoint = $endpoint
    }
}

# Is the emulator gateway serving? Prefer the HTTPS readiness endpoint (returns the cert PEM only
# once the gateway is up); the self-signed cert needs -SkipCertificateCheck (pwsh 6+). Fall back to
# a TCP reachability check on Windows PowerShell 5.1, which lacks that switch.
function Test-CosmosEmulatorUp {
    param([string]$Endpoint)

    if ($PSVersionTable.PSVersion.Major -ge 6) {
        try {
            $resp = Invoke-WebRequest -Uri "$Endpoint/_explorer/emulator.pem" -TimeoutSec 5 `
                -SkipCertificateCheck -UseBasicParsing
            return $resp.StatusCode -ge 200 -and $resp.StatusCode -lt 400
        }
        catch { return $false }
    }

    $client = $null
    try {
        $uri = [Uri]$Endpoint
        $client = [System.Net.Sockets.TcpClient]::new()
        $async = $client.BeginConnect($uri.Host, $uri.Port, $null, $null)
        if ($async.AsyncWaitHandle.WaitOne(5000) -and $client.Connected) {
            $client.EndConnect($async)
            return $true
        }
        return $false
    }
    catch { return $false }
    finally { if ($client) { $client.Dispose() } }
}

$resolved = Resolve-Persistence -SecretsFile $SecretsFile
if ($resolved.Mode -ne 'cosmos') {
    Write-Verbose 'cosmos-preflight: in-memory persistence (default) — skipping the emulator wait.'
    return
}

$endpoint = $resolved.Endpoint
if (Test-CosmosEmulatorUp -Endpoint $endpoint) {
    Write-Host "cosmos-preflight: Cosmos DB Emulator reachable at $endpoint."
    return
}

Write-Host "cosmos-preflight: Cosmos mode selected — waiting for the Cosmos DB Emulator at $endpoint (up to ${TimeoutSeconds}s)..."
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds $IntervalSeconds
    if (Test-CosmosEmulatorUp -Endpoint $endpoint) {
        Write-Host "cosmos-preflight: Cosmos DB Emulator reachable at $endpoint."
        return
    }
}

Write-Warning @"
Cosmos mode selected (TASKLIST_PERSISTENCE=cosmos) but the Cosmos DB Emulator isn't reachable at $endpoint.
  - Start it: see README > "Cosmos DB Emulator (native Windows) — optional local persistence".
  - Or use the default: remove TASKLIST_PERSISTENCE from scripts/dev-secrets.local.ps1 for in-memory.
Continuing startup — the app and liveness watch still launch, but Cosmos calls will fail until the emulator is up.
"@
return
