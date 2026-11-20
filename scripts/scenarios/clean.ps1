# Staged scenario: a quiet refactor with the suite green throughout. This is the
# control — it is supposed to produce zero incidents.
. (Join-Path $PSScriptRoot '_common.ps1')

$repo = New-ScenarioRepo 'staged-clean'
Push-Location -LiteralPath $repo

Set-Content -LiteralPath "$repo\package.json" -Value '{ "type": "module" }' -Encoding ascii
Write-Text 'src/cart.js' @'
export function total(items) {
  let sum = 0;
  for (const item of items) sum += item.price;
  return sum;
}
'@
Write-Text 'test/cart.test.js' @'
import test from 'node:test';
import assert from 'node:assert';
import { total } from '../src/cart.js';

test('adds two prices', () => {
  assert.equal(total([{ price: 2 }, { price: 3 }]), 5);
});
'@
git -C $repo add -A 2>&1 | Out-Null
git -C $repo commit -qm 'cart' 2>&1 | Out-Null

$proc = Start-Scenario -Title 'Cart: add a quantity field'

Step 'support quantity in the cart total'
Set-File 'src/cart.js' @'
export function total(items) {
  let sum = 0;
  for (const item of items) sum += item.price * (item.quantity ?? 1);
  return sum;
}
'@
Set-File 'test/cart.test.js' @'
import test from 'node:test';
import assert from 'node:assert';
import { total } from '../src/cart.js';

test('adds two prices', () => {
  assert.equal(total([{ price: 2 }, { price: 3 }]), 5);
});

test('multiplies by quantity', () => {
  assert.equal(total([{ price: 2, quantity: 4 }]), 8);
});
'@
Run @('node', '--test')
Run @('git', 'diff', '--stat')

Stop-Scenario $proc
Export-Scenario 'staged-clean'
Pop-Location
