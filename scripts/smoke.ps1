<#
Screenshots of the main states into target\smoke\<label>\, to compare before and after a change. Uses the real data folder;
"hide from capture" would make every shot black, so it is off for the run and settings.json is put back after
(or by the next run, if this one was killed).

  scripts\smoke.ps1 before
  scripts\smoke.ps1 before-fr -Language fr
#>
param(
    [Parameter(Mandatory)][string]$Label,
    [string]$Steps,
    [string]$Language
)
$out = Join-Path $PSScriptRoot "..\target\smoke\$Label"
New-Item -ItemType Directory -Force $out | Out-Null
$out = (Resolve-Path $out).Path
# Client pixels of the 1280x800 window; the account name's width moves the cog and the phone. Only views change:
# nothing is set, started or saved by a click.
if (-not $Steps) {
    $Steps = @(
        'wait:1500', "shot:$out\01-library.png",
        'wheel:760,700,-5', 'wait:500', "shot:$out\02-cheats.png", 'wheel:760,500,5',
        'click:1008,25', 'wait:900', "shot:$out\03-settings.png",
        'click:382,199', "shot:$out\04-settings-overlay.png",
        'click:382,233', "shot:$out\05-settings-hotkeys.png",
        'click:364,267', "shot:$out\06-settings-app.png",
        'click:393,301', 'wait:1500', "shot:$out\07-settings-resources.png",
        'key:27',
        'click:947,25', 'wait:700', "shot:$out\07-remote.png", 'key:27',
        'click:919,319', 'wait:3000', "shot:$out\08-map.png", 'click:20,400', 'wait:700',
        'click:1060,785', 'wait:900', "shot:$out\09-log.png", 'click:1060,785',
        'click:120,122', 'text:far', "shot:$out\10-filter.png", 'key:8', 'key:8', 'key:8',
        'click:1236,87', "shot:$out\11-sources.png", 'click:640,571', 'wait:500', "shot:$out\12-end.png"
    ) -join ';'
}
$settings = Join-Path $env:LOCALAPPDATA 'Impulse\settings.json'
$backup = "$settings.smoke"
# A run killed before its finally left the user's copy here.
if (Test-Path $backup) { Move-Item -Force $backup $settings }
try {
    if (Test-Path $settings) {
        Copy-Item $settings $backup
        $text = [IO.File]::ReadAllText($settings) -replace '"hide_from_capture"\s*:\s*true', '"hide_from_capture": false'
        if ($Language) { $text = $text -replace '"language"\s*:\s*(null|"[^"]*")', "`"language`": `"$Language`"" }
        [IO.File]::WriteAllText($settings, $text)
    }
    & (Join-Path $PSScriptRoot 'ui.ps1') -Steps $Steps
} finally {
    if (Test-Path $backup) { Move-Item -Force $backup $settings }
}
