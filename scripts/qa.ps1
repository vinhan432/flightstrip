<#
    Flightstrip QA gate.

    One command, every automated check, one PASS/FAIL table, non-zero exit on
    any FAIL. The engine checks call target\release\flightstrip.exe by absolute
    path; the engine behaviour checks (added in the next stage) run inside a
    throwaway git repo under $env:TEMP so they cannot touch this repository.

    Usage:
      powershell -ExecutionPolicy Bypass -File scripts\qa.ps1
      powershell -ExecutionPolicy Bypass -File scripts\qa.ps1 -Full     # npm ci first
      powershell -ExecutionPolicy Bypass -File scripts\qa.ps1 -SkipWeb  # engine only
#>
[CmdletBinding()]
param(
    [switch]$Full,
    [switch]$SkipRust,
    [switch]$SkipWeb
)

# Native commands legitimately write to stderr (cargo, npm). With 'Stop' that
# becomes a NativeCommandError and aborts the run, so failures are detected by
# exit code instead.
$ErrorActionPreference = 'Continue'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Exe      = Join-Path $RepoRoot 'target\release\flightstrip.exe'
$WebDir   = Join-Path $RepoRoot 'web'
$Stamp    = Get-Date -Format 'yyyyMMddTHHmmss'
$WorkRoot = Join-Path $env:TEMP "fs-qa\qa-$Stamp"

$script:Results = New-Object System.Collections.Generic.List[object]

function Write-Result {
    param([string]$Id, [string]$Name, [bool]$Ok, [string]$Detail)
    $verdict = if ($Ok) { 'PASS' } else { 'FAIL' }
    $script:Results.Add([pscustomobject]@{
        Id     = $Id
        Check  = $Name
        Result = $verdict
        Detail = $Detail
    })
    $line = "[{0}] {1,-4} {2,-44} {3}" -f $verdict, $Id, $Name, $Detail
    if ($Ok) { Write-Host $line -ForegroundColor DarkGreen }
    else     { Write-Host $line -ForegroundColor Red }
}

# Runs a native command, returns @{ Code; Output }. Never throws on exit code.
function Invoke-Native {
    param(
        [string]$File,
        [string[]]$Arguments,
        [string]$WorkingDirectory = $RepoRoot
    )
    $cwd = (Get-Location).Path
    try {
        Set-Location -LiteralPath $WorkingDirectory
        $output = (& $File @Arguments 2>&1 | Out-String)
        return @{ Code = $LASTEXITCODE; Output = $output }
    }
    finally {
        Set-Location -LiteralPath $cwd
    }
}

function Get-GzipLength {
    param([string]$Path)
    $source = [System.IO.File]::ReadAllBytes($Path)
    $buffer = New-Object System.IO.MemoryStream
    $gzip   = New-Object System.IO.Compression.GZipStream($buffer, [System.IO.Compression.CompressionLevel]::Optimal, $true)
    try { $gzip.Write($source, 0, $source.Length) } finally { $gzip.Dispose() }
    return $buffer.Length
}

function Get-ExePath {
    if (-not (Test-Path -LiteralPath $Exe)) {
        throw "release binary missing: $Exe (run cargo build --workspace --release)"
    }
    return (Resolve-Path -LiteralPath $Exe).Path
}

function New-TempRepo {
    param([string]$Name, [string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
    Push-Location -LiteralPath $Path
    try {
        git init --quiet 2>&1 | Out-Null
        git config user.email 'qa@flightstrip.local' 2>&1 | Out-Null
        git config user.name  'Flightstrip QA' 2>&1 | Out-Null
        Set-Content -LiteralPath (Join-Path $Path '.gitignore') -Value @('.flightstrip/', 'node_modules/', 'target/') -Encoding utf8
        Set-Content -LiteralPath (Join-Path $Path 'README.md') -Value "# $Name" -Encoding utf8
        git add -A 2>&1 | Out-Null
        git commit -q -m 'seed' 2>&1 | Out-Null
    }
    finally {
        Pop-Location
    }
    return $Path
}

Write-Host ""
Write-Host "Flightstrip QA  $Stamp" -ForegroundColor Cyan
Write-Host "repo   $RepoRoot"
Write-Host "work   $WorkRoot"
Write-Host ("-" * 100)

# ── environment ──────────────────────────────────────────────────────────────
foreach ($tool in @('cargo', 'node', 'npm')) {
    $probe = Invoke-Native -File $tool -Arguments @('--version')
    Write-Result 'ENV' "$tool available" ($probe.Code -eq 0) $(($probe.Output.Trim() -split "`n")[0])
}

New-Item -ItemType Directory -Path $WorkRoot -Force | Out-Null

# ── engine ───────────────────────────────────────────────────────────────────
if (-not $SkipRust) {
    $fmt = Invoke-Native -File 'cargo' -Arguments @('fmt', '--check')
    Write-Result 'R1' 'cargo fmt --check' ($fmt.Code -eq 0) $(if ($fmt.Code -eq 0) { 'clean' } else { 'formatting differs' })

    $clippy = Invoke-Native -File 'cargo' -Arguments @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
    Write-Result 'R2' 'clippy -D warnings' ($clippy.Code -eq 0) $(if ($clippy.Code -eq 0) { 'no warnings' } else { 'warnings emitted' })

    $test = Invoke-Native -File 'cargo' -Arguments @('test', '--workspace', '--quiet')
    $summary = ($test.Output -split "`n" | Select-String -Pattern 'test result:' | ForEach-Object { $_.ToString().Trim() }) -join ' | '
    Write-Result 'R3' 'cargo test --workspace' ($test.Code -eq 0) $(if ($summary) { $summary } else { 'no test summary parsed' })

    $build = Invoke-Native -File 'cargo' -Arguments @('build', '--workspace', '--release')
    if ($build.Code -eq 0 -and (Test-Path -LiteralPath $Exe)) {
        $size = (Get-Item -LiteralPath $Exe).Length
        Write-Result 'R4' 'release build' $true ("exe {0:N0} bytes" -f $size)
    }
    else {
        Write-Result 'R4' 'release build' $false 'build failed or exe missing'
    }

    if (Test-Path -LiteralPath $Exe) {
        $version = Invoke-Native -File $Exe -Arguments @('--version') -WorkingDirectory $WorkRoot
        Write-Result 'R5' 'flightstrip --version' ($version.Code -eq 0) $version.Output.Trim()

        $verifyHelp = Invoke-Native -File $Exe -Arguments @('verify', '--help') -WorkingDirectory $WorkRoot
        Write-Result 'R6' 'verify subcommand present' ($verifyHelp.Code -eq 0) 'usage printed'
    }
}

# ── web ──────────────────────────────────────────────────────────────────────
if (-not $SkipWeb) {
    if ($Full) {
        $ci = Invoke-Native -File 'npm' -Arguments @('ci') -WorkingDirectory $WebDir
        Write-Result 'W0' 'npm ci' ($ci.Code -eq 0) $(if ($ci.Code -eq 0) { 'lockfile install' } else { 'install failed' })
    }

    $webBuild = Invoke-Native -File 'npm' -Arguments @('run', 'build') -WorkingDirectory $WebDir
    Write-Result 'W1' 'npm run build' ($webBuild.Code -eq 0) $(if ($webBuild.Code -eq 0) { 'tsc + vite' } else { 'build failed' })

    if ($webBuild.Code -eq 0) {
        $js  = Get-ChildItem -LiteralPath (Join-Path $WebDir 'dist\assets') -Filter '*.js'  | Sort-Object Length -Descending | Select-Object -First 1
        $css = Get-ChildItem -LiteralPath (Join-Path $WebDir 'dist\assets') -Filter '*.css' | Sort-Object Length -Descending | Select-Object -First 1
        $jsGz  = if ($js)  { (Get-GzipLength $js.FullName) }  else { 0 }
        $cssGz = if ($css) { (Get-GzipLength $css.FullName) } else { 0 }
        $total = $jsGz + $cssGz
        Write-Result 'W2' 'gzipped JS+CSS budget' ($total -lt 200000) ("{0:N0} B / 200,000 B (js {1:N0}, css {2:N0})" -f $total, $jsGz, $cssGz)

        $index = Join-Path $WebDir 'dist\sessions\index.json'
        $sessionCount = 0
        if (Test-Path -LiteralPath $index) {
            $sessionCount = (Get-Content -LiteralPath $index -Raw | ConvertFrom-Json).sessions.Count
        }
        Write-Result 'W3' 'session catalogue shipped' ($sessionCount -gt 0) "$sessionCount sessions in dist/sessions/index.json"

        $dangling = @(Get-ChildItem -LiteralPath (Join-Path $WebDir 'dist\sessions') -Filter '*.json' |
            Where-Object { $_.Name -ne 'index.json' -and (Get-Content -LiteralPath $_.FullName -Raw) -notmatch '"steps"' })
        Write-Result 'W4' 'every shipped session has steps' ($dangling.Count -eq 0) $(if ($dangling.Count -eq 0) { 'ok' } else { "missing: $($dangling.Name -join ', ')" })
    }

    $vitest = Invoke-Native -File 'npx' -Arguments @('vitest', 'run') -WorkingDirectory $WebDir
    $esc    = [char]27   # Windows PowerShell 5.1 has no `e escape
    $plain  = $vitest.Output -replace ([regex]::Escape($esc) + '\[[0-9;]*m'), ''
    $match  = [regex]::Match($plain, 'Tests\s+(\d+) passed')
    $detail = if ($match.Success) { "$($match.Groups[1].Value) tests passed" } else { 'ran, but no "Tests N passed" line; see output' }
    Write-Result 'W5' 'web unit + integration tests' ($vitest.Code -eq 0) $detail
}

# ── report ───────────────────────────────────────────────────────────────────
$fail = @($script:Results | Where-Object { $_.Result -eq 'FAIL' })
Write-Host ("-" * 100)
Write-Host ("{0} checks, {1} passed, {2} failed" -f $script:Results.Count, ($script:Results.Count - $fail.Count), $fail.Count)
if ($fail.Count -gt 0) {
    Write-Host "FAILED:" -ForegroundColor Red
    foreach ($f in $fail) { Write-Host ("  {0} {1} - {2}" -f $f.Id, $f.Check, $f.Detail) -ForegroundColor Red }
    exit 1
}
Write-Host "GATE: PASS" -ForegroundColor Green
exit 0
