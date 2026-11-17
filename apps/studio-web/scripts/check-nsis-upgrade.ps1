# The update path of the Windows installer (F-820, ADR 0801 Addendum 19, Q-F820-3): an update is the next installer run
# over the installed copy. This installs the NSIS build silently (per user), puts a marker file in the user's settings
# folder (%APPDATA%\rcfd Studio, Electron's userData, named after productName), installs again over it (the same build,
# or -Previous first and then -Installer), and checks that the marker and a recent list survive, that the same copy was
# upgraded in place (one uninstall entry, the same folder), that the installed app starts (RCFD_STUDIO_STARTUP_REPORT:
# the renderer's ready report, then it quits) and that the engine is in <install>\resources\bin. Then it uninstalls
# silently and checks the settings folder is still there (deleteAppDataOnUninstall: false). Before the uninstall,
# scripts/check-installed.mjs runs on the installed copy: with no RCFD_STUDIO_* variable it uses the shipped engine and
# starts its service with the first project. The marker is removed at the end.
#
#   pwsh scripts/check-nsis-upgrade.ps1 -Installer out\installers\rcfd-studio-X.Y.Z-setup-x64.exe [-Previous <older setup>]
#                                       [-InstallDir <folder without spaces>] [-KeepInstalled]
#
# Meant for a CI runner (studio-installers.yml): it installs into the profile it runs in. Exit code 0 when every check
# holds. -KeepInstalled skips the uninstall; -SkipEngineCheck skips check-installed.mjs (it needs npm ci's Playwright).
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [string]$Previous = '',
  [string]$InstallDir = '',
  [switch]$KeepInstalled,
  [switch]$SkipEngineCheck
)
$ErrorActionPreference = 'Stop'
$failures = [System.Collections.Generic.List[string]]::new()
function Check([bool]$ok, [string]$what) {
  if ($ok) { Write-Host "ok   $what" } else { Write-Host "FAIL $what"; $failures.Add($what) }
}
function Entries {
  Get-ChildItem HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall -ErrorAction SilentlyContinue |
    ForEach-Object { Get-ItemProperty $_.PSPath } | Where-Object { $_.DisplayName -like 'rcfd Studio*' }
}
function Install([string]$setup) {
  # NSIS takes /D= last and unquoted: one argument string, so PowerShell adds no quotes
  $argLine = if ($InstallDir) { "/S /D=$InstallDir" } else { '/S' }
  $p = Start-Process -FilePath $setup -ArgumentList $argLine -Wait -PassThru
  Check ($p.ExitCode -eq 0) "silent install of $(Split-Path $setup -Leaf) (exit code $($p.ExitCode))"
}
function InstalledExe {
  $e = @(Entries)
  if ($e.Count -ne 1) { return $null }
  $dir = $e[0].InstallLocation
  if (-not $dir) { $dir = Split-Path ($e[0].UninstallString -replace '^"([^"]+)".*$', '$1') }
  return Join-Path $dir 'rcfd Studio.exe'
}

$Installer = (Resolve-Path $Installer).Path
if ($Previous) { $Previous = (Resolve-Path $Previous).Path }
Check (@(Entries).Count -eq 0) 'no rcfd Studio is installed before the check'
if ($failures.Count) { Write-Host 'an installed rcfd Studio would be replaced and removed: stopping'; exit 1 }

$userData = Join-Path $env:APPDATA 'rcfd Studio'
$marker = Join-Path $userData "f820-upgrade-marker-$PID.txt"
$recent = Join-Path $userData 'recent-projects.json'
$hadRecent = Test-Path $recent
$scratch = Join-Path ([System.IO.Path]::GetTempPath()) "rcfd-f820-upgrade-$PID"
New-Item -ItemType Directory -Force $scratch | Out-Null
try {
  # 1. the first copy (the previous build when given, else this one)
  Install ($(if ($Previous) { $Previous } else { $Installer }))
  $exe = InstalledExe
  Check ($null -ne $exe -and (Test-Path $exe)) "installed: $exe"
  $firstDir = Split-Path $exe
  if (-not $InstallDir) {
    $expected = Join-Path $env:LOCALAPPDATA 'Programs\rcfd Studio'
    Check ($firstDir -eq $expected) "the per-user install folder is $expected"
  }

  # 2. the user's settings: a marker, and a recent list when there is none
  New-Item -ItemType Directory -Force $userData | Out-Null
  Set-Content -Path $marker -Value "written before the upgrade $(Get-Date -Format o)" -NoNewline
  if (-not $hadRecent) { Set-Content -Path $recent -Value '[]' -NoNewline }

  # 3. the update: this installer over the installed copy
  Install $Installer
  $exe = InstalledExe
  Check (@(Entries).Count -eq 1) 'one uninstall entry after the upgrade (the same appId, upgraded in place)'
  Check ($null -ne $exe -and (Split-Path $exe) -eq $firstDir) "the same folder after the upgrade ($firstDir)"
  Check (Test-Path $marker) 'the marker in the settings folder survived the upgrade'
  Check (Test-Path $recent) 'the recent list survived the upgrade'
  foreach ($n in 'rcfd.exe', 'rcfd-mcp.exe', 'rcfd-runner.exe') {
    Check (Test-Path (Join-Path $firstDir "resources\bin\$n")) "the engine is installed: resources\bin\$n"
  }
  Check (-not (Test-Path (Join-Path $firstDir 'resources\app\node_modules'))) 'no node_modules in the installed app'

  # 4. the upgraded app starts: the renderer reports ready, then it quits (the P10 hook of src/main/index.ts)
  $report = Join-Path $scratch 'ready.json'
  $env:RCFD_STUDIO_STARTUP_REPORT = $report
  try {
    $p = Start-Process -FilePath $exe -PassThru
    if (-not $p.WaitForExit(60000)) { $p.Kill($true); Check $false 'the upgraded app quit within 60 s after its ready report' }
  } finally {
    Remove-Item Env:RCFD_STUDIO_STARTUP_REPORT
  }
  Check (Test-Path $report) "the upgraded app started and its page reported ready$(if (Test-Path $report) { ': ' + (Get-Content $report -Raw) })"

  # 5. the installed copy uses the engine it ships with (no variable set) and starts its service with a project
  if (-not $SkipEngineCheck) {
    & node (Join-Path $PSScriptRoot 'check-installed.mjs') "--exe=$exe"
    Check ($LASTEXITCODE -eq 0) 'the installed Studio uses its shipped engine (check-installed.mjs)'
  }

  if (-not $KeepInstalled) {
    # 6. uninstall: the program goes, the settings stay
    # the entry's own silent command line: "<folder>\Uninstall rcfd Studio.exe" /currentuser /S
    $quiet = @(Entries)[0].QuietUninstallString
    $u = $quiet -replace '^"([^"]+)".*$', '$1'
    $uArgs = $quiet -replace '^"[^"]+"\s*', ''
    Check ((Split-Path $u) -eq $firstDir -and (Test-Path $u)) "the uninstaller is there ($u $uArgs)"
    $p = Start-Process -FilePath $u -ArgumentList $uArgs -Wait -PassThru
    # the NSIS uninstaller copies itself to a temporary folder and runs from there: wait for the entry to go
    $deadline = (Get-Date).AddSeconds(60)
    while (@(Entries).Count -gt 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    Check (@(Entries).Count -eq 0) "silent uninstall (exit code $($p.ExitCode)): no uninstall entry left"
    Check (-not (Test-Path $exe)) 'the program is gone'
    Check (Test-Path $marker) 'the settings folder is kept on uninstall (deleteAppDataOnUninstall: false)'
  }
} finally {
  Remove-Item -Force $marker -ErrorAction SilentlyContinue
  if (-not $hadRecent) { Remove-Item -Force $recent -ErrorAction SilentlyContinue }
  Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
}
if ($failures.Count) { Write-Host "$($failures.Count) check(s) failed"; exit 1 }
Write-Host 'every check holds'
