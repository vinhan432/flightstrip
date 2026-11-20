# Staged scenario: three packages added to a manifest, then a token pasted into
# source and a .env created. The token is a fake, shape-correct value.
. (Join-Path $PSScriptRoot '_common.ps1')

$repo = New-ScenarioRepo 'staged-dependency-secret'
Push-Location -LiteralPath $repo

Write-Text 'package.json' @'
{
  "name": "checkout",
  "version": "1.0.0",
  "dependencies": {
    "express": "4.19.2"
  }
}
'@
Write-Text 'src/client.js' @'
export const endpoint = 'https://api.example.test/v1';
'@
git -C $repo add -A 2>&1 | Out-Null
git -C $repo commit -qm 'checkout client' 2>&1 | Out-Null

$proc = Start-Scenario -Title 'Auth: wire the webhook client'

Step 'add retries, logging and a date library'
Set-File 'package.json' @'
{
  "name": "checkout",
  "version": "1.0.0",
  "dependencies": {
    "express": "4.19.2",
    "p-retry": "6.2.0",
    "pino": "9.2.0",
    "dayjs": "1.11.11"
  }
}
'@

Step 'use the token from the dashboard'
Set-File 'src/client.js' @'
// TODO: move this to an environment variable before shipping
export const token = 'ghp_abcdefghijklmnopqrstuvwxyz0123456789';
export const endpoint = 'https://api.example.test/v1';
'@

Step 'put the signing key in .env'
Write-Text '.env' "STRIPE_SECRET=sk-test1234567890abcdefghijklmnop`n"
Start-Sleep -Milliseconds 900

Stop-Scenario $proc
Export-Scenario 'staged-dependency-secret'
Pop-Location
