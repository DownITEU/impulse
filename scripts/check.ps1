<#
The gate before work is called done: formatting, clippy with warnings denied, every test, and the panel's types, lint
and tests once it is installed (scripts\panel.ps1). Stops at the first failure.

  scripts\check.ps1
#>
Set-Location (Join-Path $PSScriptRoot '..')
# A missing cargo leaves $LASTEXITCODE at 0, so it would pass.
Get-Command cargo -ErrorAction Stop | Out-Null
$steps = @(
    @('fmt', '--all', '--', '--check'),
    @('clippy', '--all-targets', '--', '-D', 'warnings'),
    @('clippy', '-p', 'impulse', '--no-default-features', '--', '-D', 'warnings'),
    @('test', '--quiet')
)
foreach ($step in $steps) {
    "> cargo $($step -join ' ')"
    & cargo @step
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (Test-Path 'web\panel\node_modules') {
    '> pnpm verify (web\panel)'
    pnpm --dir web/panel verify
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} else {
    'panel not installed: its checks skipped (scripts\panel.ps1)'
}
'all checks passed'
