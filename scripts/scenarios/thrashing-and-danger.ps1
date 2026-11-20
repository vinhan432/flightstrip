# Staged scenario: one file rewritten six times, then two dangerous commands.
#
# The rewrites are recorded for real. The dangerous commands are NOT executed:
# they are appended to the session log as `command` events with the shape the
# recorder writes, because running `rm -rf /` to prove a detector works would be
# reckless. The export is labelled `fixture` so nothing claims the shell ran
# them.
. (Join-Path $PSScriptRoot '_common.ps1')

$repo = New-ScenarioRepo 'staged-thrash-danger'
Push-Location -LiteralPath $repo

Set-Content -LiteralPath "$repo\package.json" -Value '{ "type": "module" }' -Encoding ascii
Write-Text 'src/webhook.js' @'
export function verify(signature, body) {
  return signature === body;
}
'@
git -C $repo add -A 2>&1 | Out-Null
git -C $repo commit -qm 'webhook' 2>&1 | Out-Null

$proc = Start-Scenario -Title 'Auth: ship the webhook signature check'

Step 'make the signature check real'
Set-File 'src/webhook.js' "export function verify(signature, body) { return signature.length > 0; }`n"
Set-File 'src/webhook.js' "export function verify(signature, body) { return signature.startsWith('sha256='); }`n"
Set-File 'src/webhook.js' "export function verify(signature, body) { return signature === body; }`n"
Set-File 'src/webhook.js' "export function verify(signature, body) { return signature === body.trim(); }`n"
Set-File 'src/webhook.js' "export function verify(signature, body) { return body.trim() === signature; }`n"
Set-File 'src/webhook.js' "export function verify(signature, body) { return signature === body; }`n"

Step 'sync the branch'
Run @('git', 'status', '--short')

Stop-Scenario $proc

# Append the two commands that must never be run for real, with continuing seq
# and t_ms, then relabel the export as a fixture.
$sessionDir = Get-ChildItem -LiteralPath '.flightstrip\sessions' -Directory |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
$logPath = Join-Path $sessionDir.FullName 'events.jsonl'
$lines = @(Get-Content -LiteralPath $logPath | Where-Object { $_.Trim() })
$last = $lines[-1] | ConvertFrom-Json
$seq = [int]$last.seq + 1
$t = [int]$last.t_ms + 1200

$injected = @(
    [ordered]@{ v = 1; seq = $seq; t_ms = $t; kind = 'command'; cmd = 'git push --force origin main'; exit_code = 0; duration_ms = 340; source = 'exec'; cwd = ''; prompt_id = 'p-2' },
    [ordered]@{ v = 1; seq = $seq + 1; t_ms = $t + 900; kind = 'command'; cmd = 'rm -rf /'; exit_code = 1; duration_ms = 12; source = 'exec'; cwd = ''; prompt_id = 'p-2' }
)
$injected | ForEach-Object { $lines += ($_ | ConvertTo-Json -Compress) }
[System.IO.File]::WriteAllText($logPath, (($lines -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding($false)))

Export-Scenario 'staged-thrash-danger'
Pop-Location

# Relabel to fixture: two of these commands were written, not run.
$path = Join-Path $script:RepoRoot 'fixtures\staged\staged-thrash-danger.json'
$text = Get-Content -LiteralPath $path -Raw
$text = $text -replace '"label": "recorded"', '"label": "fixture"'
[System.IO.File]::WriteAllText($path, $text, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "  relabelled as fixture: two commands are injected, not executed"
