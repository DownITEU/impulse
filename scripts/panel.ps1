<#
Builds the remote panel into web\panel\dist, which the next cargo build embeds. Needs Node and pnpm.

  scripts\panel.ps1         # install, then build
  scripts\panel.ps1 -Dev    # the dev server: http://127.0.0.1:4173/?demo shows sample views
#>
param([switch]$Dev)
Set-Location (Join-Path $PSScriptRoot '..\web\panel')
Get-Command pnpm -ErrorAction Stop | Out-Null
pnpm install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if ($Dev) { pnpm dev } else { pnpm build }
exit $LASTEXITCODE
