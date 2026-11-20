# Shared plumbing for the staged scenario scripts.
#
# Each scenario records a real session with the real binary in a throwaway repo,
# then exports it to fixtures/staged/<name>.json. Nothing here invents events:
# the only thing a scenario does is type files and run commands while
# `flightstrip watch` is recording.
#
# The recorder is stopped with Stop-Process, so the log has no session_end and
# no Ctrl+C summary. The exported data is exactly what was recorded; the missing
# end marker is the one artefact of being scripted, and it is stated in the
# catalogue blurb for every staged session.

$ErrorActionPreference = 'Continue'

$script:RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$script:Exe = Join-Path $script:RepoRoot 'target\release\flightstrip.exe'
$script:Stamp = Get-Date -Format 'yyyyMMddTHHmmss'
$script:Work = Join-Path $env:TEMP "fs-qa\scenario-$script:Stamp"

function Assert-Binary {
    if (-not (Test-Path -LiteralPath $script:Exe)) {
        throw "release binary missing: $script:Exe (cargo build --workspace --release)"
    }
}

function New-ScenarioRepo {
    param([string]$Name)
    Assert-Binary
    $path = Join-Path $script:Work $Name
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    Push-Location -LiteralPath $path
    git init -q 2>&1 | Out-Null
    git config user.email 'demo@flightstrip.local'
    git config user.name 'Flightstrip demo'
    Write-Text '.gitignore' ".flightstrip/`nnode_modules/`n"
    Write-Text 'README.md' "# $Name`n"
    git add -A 2>&1 | Out-Null
    git commit -qm seed 2>&1 | Out-Null
    Pop-Location
    return $path
}

function Write-Text {
    param([string]$Path, [string]$Text)
    $full = if ([System.IO.Path]::IsPathRooted($Path)) { $Path } else { Join-Path (Get-Location).Path $Path }
    $dir = Split-Path -Parent $full
    if ($dir -and -not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    [System.IO.File]::WriteAllText($full, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Start-Scenario {
    param([string]$Title)
    $stdout = Join-Path $script:Work "$Title.out.log"
    $stderr = Join-Path $script:Work "$Title.err.log"
    # Start-Process does not quote array elements, so a title with spaces would
    # arrive as five positional arguments and watch would refuse to start.
    $argLine = 'watch --title "{0}" --agent "{1}" --interval 120' -f $Title, 'claude code'
    $proc = Start-Process -FilePath $script:Exe `
        -ArgumentList $argLine `
        -WorkingDirectory (Get-Location).Path -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path -LiteralPath '.flightstrip\active') { break }
        Start-Sleep -Milliseconds 100
    }
    Start-Sleep -Milliseconds 700
    return $proc
}

function Stop-Scenario {
    param($Process)
    if ($Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force
        $Process.WaitForExit(5000) | Out-Null
    }
    Start-Sleep -Milliseconds 200
}

function Step {
    param([string]$Prompt)
    & $script:Exe mark $Prompt | Out-Null
    Start-Sleep -Milliseconds 250
}

function Run {
    param([string[]]$Command)
    & $script:Exe exec -- @Command | Out-Null
    Start-Sleep -Milliseconds 400
}

function Set-File {
    param([string]$Path, [string]$Text)
    Write-Text -Path $Path -Text $Text
    Start-Sleep -Milliseconds 900
}

# Exports the newest session to fixtures/staged/<Name>.json and prints a summary.
function Export-Scenario {
    param([string]$Name)
    $out = Join-Path $script:RepoRoot "fixtures\staged\$Name.json"
    New-Item -ItemType Directory -Path (Split-Path -Parent $out) -Force | Out-Null
    & $script:Exe export --out $out | Out-Null
    if (-not (Test-Path -LiteralPath $out)) { throw "export produced no file for $Name" }

    $session = Get-Content -LiteralPath $out -Raw | ConvertFrom-Json
    $kinds = @($session.incidents | ForEach-Object { $_.kind } | Sort-Object -Unique) -join ', '
    Write-Host ("{0}: {1} steps, {2} incidents [{3}]" -f $Name, $session.steps.Count, $session.incidents.Count, $kinds)
    Write-Host ("  -> {0}" -f $out)
    return $out
}
