# Build the Studio, the run service, the solver and the MCP server in release, then start the Studio (ADR 0141).
#
#   powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 [-NoBuild] [-DryRun] [PROJECT_DIR]
#
# -NoBuild skips the build. -DryRun prints the two commands, one argument per line ("build> ", "start> "), and runs
# nothing. CARGO names the cargo program (default: cargo); CARGO_TARGET_DIR is honoured. The project folder is passed
# to the Studio as given (relative to the current folder, which the Studio keeps).
$usage = 'usage: studio.ps1 [-NoBuild] [-DryRun] [PROJECT_DIR]'
$build = $true
$dry = $false
$project = $null
foreach ($a in $args) {
    $s = [string]$a
    if ($s -in @('-NoBuild', '--no-build')) { $build = $false }
    elseif ($s -in @('-DryRun', '--dry-run')) { $dry = $true }
    elseif ($s -in @('-h', '-Help', '--help', '-?', '/?')) { Write-Output $usage; exit 0 }
    elseif ($s.Length -gt 1 -and $s.StartsWith('-')) {
        [Console]::Error.WriteLine("studio.ps1: unknown option '$s'`n$usage")
        exit 2
    }
    elseif ($null -ne $project) {
        [Console]::Error.WriteLine("studio.ps1: expected at most one project folder, got '$project' and '$s'`n$usage")
        exit 2
    }
    else { $project = $s }
}
$root = Split-Path -Parent $PSScriptRoot
$cargo = if ($env:CARGO) { $env:CARGO } else { 'cargo' }
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
$suffix = if ($PSVersionTable.PSEdition -eq 'Core' -and -not $IsWindows) { '' } else { '.exe' }
$exe = Join-Path (Join-Path $target 'release') "rcfd-studio$suffix"
$buildArgs = @('build', '--manifest-path', (Join-Path $root 'Cargo.toml'), '--release',
    '-p', 'rcfd-studio', '-p', 'rcfd-runner', '-p', 'rcfd-cli', '-p', 'rcfd-mcp')
$startArgs = @()
if ($null -ne $project) { $startArgs = @($project) }
if ($dry) {
    if ($build) { "build> $cargo"; foreach ($x in $buildArgs) { "build> $x" } }
    "start> $exe"
    foreach ($x in $startArgs) { "start> $x" }
    exit 0
}
if ($build) {
    & $cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        [Console]::Error.WriteLine("studio.ps1: the build failed (exit $LASTEXITCODE); the Studio is not started")
        exit $LASTEXITCODE
    }
}
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    [Console]::Error.WriteLine("studio.ps1: $exe not found; build it first (leave out -NoBuild)")
    exit 1
}
& $exe @startArgs
exit $LASTEXITCODE
