# KissLight - delivery gate: fmt / clippy -D warnings / tests (ASCII only).
#
#   build\check.ps1
#
# All three steps must exit 0. Tests include the hygiene gate (no file writes,
# registry writes only in core/gamma.rs, ...) and the docs consistency gate.

$ErrorActionPreference = "Stop"

$src = Split-Path -Parent $PSScriptRoot
$root = Split-Path -Parent $src
$env:CARGO_TARGET_DIR = Join-Path $root ".temp\target"

Push-Location $src
try {
    Write-Host "[1/3] cargo fmt --check" -ForegroundColor Cyan
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw "format check failed (run: cargo fmt)" }

    Write-Host "[2/3] cargo clippy --all-targets -- -D warnings" -ForegroundColor Cyan
    cargo clippy --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "clippy reported warnings" }

    Write-Host "[3/3] cargo test" -ForegroundColor Cyan
    cargo test
    if ($LASTEXITCODE -ne 0) { throw "tests failed" }

    Write-Host ""
    Write-Host "GATE PASSED" -ForegroundColor Green
} finally {
    Pop-Location
}
