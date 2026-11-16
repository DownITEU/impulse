<#
The gate before work is called done: formatting, clippy with warnings denied, every test. Stops at the first failure.

  scripts\check.ps1
#>
Set-Location (Join-Path $PSScriptRoot '..')
# A missing cargo leaves $LASTEXITCODE at 0, so it would pass.
Get-Command cargo -ErrorAction Stop | Out-Null
$steps = @(
    @('fmt', '--all', '--', '--check'),
    @('clippy', '--all-targets', '--', '-D', 'warnings'),
    # The build without the remote panel's server.
    @('clippy', '-p', 'impulse', '--no-default-features', '--', '-D', 'warnings'),
    @('test', '--quiet')
)
foreach ($step in $steps) {
    "> cargo $($step -join ' ')"
    & cargo @step
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
'all checks passed'
