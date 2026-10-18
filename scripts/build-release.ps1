$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Set-Location (Split-Path -Parent $PSScriptRoot)
function Run-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}
if ((git status --porcelain | Out-String).Trim()) { throw 'Commit intended changes before building a public release.' }
$sourceCommit = (git rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'A source commit is required.' }
Run-Checked 'node' @('scripts/check-release.mjs')
Run-Checked 'npm.cmd' @('test')
Run-Checked 'cargo' @('fmt', '--manifest-path', 'src-tauri/Cargo.toml', '--check')
Run-Checked 'cargo' @('test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml')
Run-Checked 'cargo' @('clippy', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', '--all-targets', '--', '-D', 'warnings')
$savedEncoded = [Environment]::GetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', 'Process')
$savedRust = [Environment]::GetEnvironmentVariable('RUSTFLAGS', 'Process')
try {
    $workspacePath = (Get-Location).Path
    $homePath = [Environment]::GetFolderPath('UserProfile')
    $flags = @()
    if ($savedEncoded) { $flags += $savedEncoded.Split([char]31) }
    elseif ($savedRust) { throw 'Use CARGO_ENCODED_RUSTFLAGS for additional release flags; RUSTFLAGS cannot be safely split.' }
    foreach ($mapping in @(@($homePath, '/build-user'), @($workspacePath, '/workspace/local-agent'))) {
        $flags += "--remap-path-prefix=$($mapping[0])=$($mapping[1])"
        $flags += "--remap-path-prefix=$($mapping[0].Replace('\','/'))=$($mapping[1])"
    }
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]31
    Run-Checked 'npm.cmd' @('run', 'package')
} finally {
    [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', $savedEncoded, 'Process')
    [Environment]::SetEnvironmentVariable('RUSTFLAGS', $savedRust, 'Process')
}
$version = (Get-Content 'package.json' -Raw | ConvertFrom-Json).version
$installer = "src-tauri/target/release/bundle/nsis/Local Agent_${version}_x64-setup.exe"
Run-Checked 'node' @('scripts/check-release.mjs', '--binary', 'src-tauri/target/release/local-agent.exe', $installer)
if ((git status --porcelain | Out-String).Trim()) { throw 'Build changed source files; review, commit and rebuild.' }
if ((git rev-parse HEAD).Trim() -ne $sourceCommit) { throw 'Source commit changed during release build.' }
$outputPath = Join-Path 'release-artifacts' "v$version"
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
$installerName = Split-Path -Leaf $installer
Copy-Item -LiteralPath $installer -Destination (Join-Path $outputPath $installerName)
$digest = (Get-FileHash -Algorithm SHA256 -LiteralPath $installer).Hash.ToLowerInvariant()
[IO.File]::WriteAllText((Join-Path (Get-Location) "$outputPath/SHA256SUMS"), "$digest  $installerName`n", [Text.UTF8Encoding]::new($false))
$manifest = [ordered]@{ version = $version; commit = $sourceCommit; builtAt = [DateTime]::UtcNow.ToString('o'); installer = $installerName; sha256 = $digest; node = (node --version); rust = (rustc --version); productionPipeline = 'Tauri 2 / NSIS'; signed = $false }
$manifest | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $outputPath 'release-manifest.json')
Write-Output "Release candidate ready: $outputPath ($sourceCommit)"
