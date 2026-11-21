<#
Checks a release tag against the workspace version and takes its changelog section, the text of the release page.
A pre-release tag reads its base version's section. Omit -Output to print the notes.

  # $tag is the workspace version prefixed with v.
  scripts\release-notes.ps1 -Tag $tag -Output release-notes.md
#>
param(
    [Parameter(Mandatory)][string]$Tag,
    [string]$Output
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if ($Tag -notmatch '^v\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$') { throw "Tag '$Tag' is not v<major>.<minor>.<patch>, or with a -pre suffix." }
$version = $Tag.Substring(1)

$manifest = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($manifest -notmatch '(?ms)\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') { throw 'No workspace version in Cargo.toml.' }
# The tag and the build name one version: an executable that says otherwise would be offered its own update.
if ($Matches[1] -cne $version) { throw "Tag $Tag does not match Cargo.toml's version $($Matches[1])." }

$base = $version -replace '-.*$', ''
$text = (Get-Content (Join-Path $root 'CHANGELOG.md') -Raw) -replace "`r`n", "`n"
$section = [regex]::Match($text, "(?ms)^## \[$([regex]::Escape($base))\][^\n]*\n(.*?)(?=^## \[|\z)")
if (-not $section.Success -or [string]::IsNullOrWhiteSpace($section.Groups[1].Value)) { throw "CHANGELOG.md has no section for $base." }
$notes = $section.Groups[1].Value.Trim()

if ($Output) { [IO.File]::WriteAllText([IO.Path]::Combine((Get-Location).Path, $Output), "$notes`n") } else { $notes }
