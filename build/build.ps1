# build.ps1 - KissLight 构建脚本
#
# 用法：
#   .\build.ps1            # Release 构建
#   .\build.ps1 -Debug     # Debug 构建
#   .\build.ps1 -Clean     # 清理编译缓存

param(
    [switch]$Debug,
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

# 路径设置
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SrcDir = Split-Path -Parent $ScriptDir
$ProjectRoot = (Get-Item $SrcDir).Parent.FullName

# 清理
if ($Clean) {
    Write-Host "清理编译缓存..." -ForegroundColor Yellow
    $td = Join-Path $ProjectRoot ".build\target"
    if (Test-Path $td) {
        Remove-Item -Path $td -Recurse -Force
    }
    Write-Host "清理完成" -ForegroundColor Green
    exit 0
}

# 构建
$p = if ($Debug) { "debug" } else { "release" }
Write-Host "构建 ($p)..." -ForegroundColor Cyan

Push-Location $SrcDir
try {
    if ($Debug) {
        cargo build
    } else {
        cargo build --release
    }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    # 复制 exe 到 .build/ 并重命名
    $td = Join-Path $ProjectRoot ".build\target"
    $src = Join-Path $td "$p\KissLight_x64.exe"
    $dst = Join-Path $ProjectRoot ".build\KissLight_x64_1.0.1.exe"
    if (Test-Path $src) {
        Copy-Item $src $dst -Force
        $sz = (Get-Item $dst).Length
        Write-Host "构建成功: $dst ($([math]::Round($sz/1KB))KB)" -ForegroundColor Green
    } else {
        Write-Host "警告: 未找到 $src" -ForegroundColor Yellow
    }
} finally {
    Pop-Location
}
