# KissLight - release build + deploy (ASCII only on purpose: PowerShell 5.1 encoding).
#
#   build\build.ps1
#
# Steps: cargo build --release -> copy exe to <root>\.build\KissLight.exe
#        -> copy LICENSE + third-party license -> check VERSIONINFO -> MD5/size report.
# Nothing is deleted here: to clean, remove <root>\.temp\target manually.

$ErrorActionPreference = "Stop"

$src = Split-Path -Parent $PSScriptRoot
$root = Split-Path -Parent $src
$target = Join-Path $root ".temp\target"
$out = Join-Path $root ".build"

$env:CARGO_TARGET_DIR = $target

# Version comes from Cargo.toml [package] - single source of truth.
$version = $null
$inPackage = $false
foreach ($line in Get-Content (Join-Path $src "Cargo.toml")) {
    $t = $line.Trim()
    if ($t.StartsWith("[")) { $inPackage = ($t -eq "[package]"); continue }
    if ($inPackage -and $t.StartsWith("version = ")) {
        $version = $t.Split('"')[1]
        break
    }
}
if (-not $version) { throw "version not found in Cargo.toml [package]" }

Push-Location $src
try {
    Write-Host "[1/3] cargo build --release" -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally {
    Pop-Location
}

$built = Join-Path $target "release\KissLight.exe"
if (-not (Test-Path $built)) { throw "missing build output: $built" }

Write-Host "[2/3] deploy to $out" -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $out | Out-Null
$dst = Join-Path $out "KissLight.exe"
Copy-Item $built $dst -Force
Copy-Item (Join-Path $src "LICENSE") (Join-Path $out "LICENSE") -Force
Copy-Item (Join-Path $src "licenses\win-nightlight-lib-MIT.txt") (Join-Path $out "LICENSE-win-nightlight-lib") -Force

Write-Host "[3/3] verify" -ForegroundColor Cyan
$info = (Get-Item $dst).VersionInfo
if ($info.FileVersion -ne $version) {
    throw "VERSIONINFO mismatch: exe reports '$($info.FileVersion)', Cargo.toml says '$version'"
}
$srcHash = (Get-FileHash -Algorithm MD5 $built).Hash
$dstHash = (Get-FileHash -Algorithm MD5 $dst).Hash
$srcSize = (Get-Item $built).Length
$dstSize = (Get-Item $dst).Length
Write-Host ("built   : {0} bytes  md5={1}" -f $srcSize, $srcHash)
Write-Host ("shipped : {0} bytes  md5={1}" -f $dstSize, $dstHash)
if ($srcHash -ne $dstHash -or $srcSize -ne $dstSize) { throw "deployed copy differs from build output" }
Write-Host ("version : {0}  (FileVersion/ProductVersion)" -f $info.FileVersion)

Write-Host ""
Write-Host "artifacts in ${out}:" -ForegroundColor Green
Get-ChildItem $out | Sort-Object Name | Format-Table Name, Length, LastWriteTime -AutoSize
