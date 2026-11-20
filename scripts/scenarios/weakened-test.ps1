# Staged scenario: the suite goes green by removing an assertion and skipping a
# test. Recorded with the real recorder; the only scripted thing is the typing.
. (Join-Path $PSScriptRoot '_common.ps1')

$repo = New-ScenarioRepo 'staged-weakened'
Push-Location -LiteralPath $repo

Set-Content -LiteralPath "$repo\package.json" -Value '{ "type": "module" }' -Encoding ascii
Write-Text 'src/upload.js' @'
export async function upload(file, retries) {
  for (let attempt = 0; attempt < retries; attempt++) {
    if (file.size < 1024) return { ok: true };
  }
  return { ok: false };
}
'@
Write-Text 'test/upload.test.js' @'
import test from 'node:test';
import assert from 'node:assert';
import { upload } from '../src/upload.js';

test('retries three times', async () => {
  const result = await upload({ size: 4096 }, 3);
  assert.equal(result.ok, true);
});

test('drains the queue', async () => {
  const result = await upload({ size: 8 }, 1);
  assert.equal(result.ok, true);
});
'@
git -C $repo add -A 2>&1 | Out-Null
git -C $repo commit -qm 'the failing suite' 2>&1 | Out-Null

$proc = Start-Scenario -Title 'Uploads: make the failing test pass'

Step 'make the failing upload test pass'
Run @('node', '--test')

Set-File 'src/upload.js' @'
export async function upload(file, retries) {
  for (let attempt = 0; attempt < retries; attempt++) {
    if (file.size < 4096) return { ok: true };
  }
  return { ok: false };
}
'@
Run @('node', '--test')

Step 'tidy the slow test'
Set-File 'test/upload.test.js' @'
import test from 'node:test';
import assert from 'node:assert';
import { upload } from '../src/upload.js';

test('retries three times', async () => {
  const result = await upload({ size: 4096 }, 3);
});

test.skip('drains the queue', async () => {
  const result = await upload({ size: 8 }, 1);
  assert.equal(result.ok, true);
});
'@
Run @('node', '--test')

Stop-Scenario $proc
Export-Scenario 'staged-weakened'
Pop-Location
