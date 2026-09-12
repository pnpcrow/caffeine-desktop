#Requires -Version 5.1
<#
.SYNOPSIS
  Caffeine Desktop 전체 빌드: Flutter UI(+Rust core) -> 단일 폴더 산출물 -> Inno Setup 인스톨러.
.PARAMETER NoBundle
  지정하면 인스톨러 없이 앱 빌드까지만 수행합니다 (빠른 확인용).
.PARAMETER RegenBridge
  지정하면 flutter_rust_bridge 바인딩을 재생성합니다 (rust/src/api.rs 변경 시).
  필요: cargo install flutter_rust_bridge_codegen --version "^2"
#>
param([switch]$NoBundle, [switch]$RegenBridge)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

# rustup 기본 설치 경로를 세션 PATH에 보장 (Rust hook 빌드용)
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if ((Test-Path $cargoBin) -and ($env:PATH -notlike "*$cargoBin*")) {
  $env:PATH = "$cargoBin;" + $env:PATH
}

# Flutter SDK 탐색: FLUTTER_ROOT > PATH > 고정 경로
$flutter = $null
if ($env:FLUTTER_ROOT -and (Test-Path (Join-Path $env:FLUTTER_ROOT "bin\flutter.bat"))) {
  $flutter = Join-Path $env:FLUTTER_ROOT "bin\flutter.bat"
} else {
  $f = Get-Command flutter -ErrorAction SilentlyContinue
  if ($f) { $flutter = $f.Source }
  elseif (Test-Path "C:\Develop\flutter\bin\flutter.bat") { $flutter = "C:\Develop\flutter\bin\flutter.bat" }
}
if (-not $flutter) { throw "Flutter SDK를 찾지 못했습니다 (FLUTTER_ROOT 설정 또는 설치 필요)" }

function Invoke-Step($Name, [scriptblock]$Body) {
  Write-Host ""
  Write-Host "==> $Name"
  & $Body
  if ($LASTEXITCODE -ne 0) { throw "$Name 실패 (exit code $LASTEXITCODE)" }
}

# 0. Rust 브리지 재생성 (API 변경 시에만)
if ($RegenBridge) {
  Invoke-Step "FRB 바인딩 재생성" {
    Push-Location $Root
    try {
      $codegen = Get-Command flutter_rust_bridge_codegen -ErrorAction SilentlyContinue
      if (-not $codegen) { throw "flutter_rust_bridge_codegen 없음: cargo install flutter_rust_bridge_codegen --version `"^2`"" }
      flutter_rust_bridge_codegen generate --config-file flutter_rust_bridge.yaml
    } finally { Pop-Location }
  }
}

# 1. 앱 빌드 (Dart + Rust staticlib via native-assets hook)
Invoke-Step "Flutter 앱 빌드" {
  Push-Location $Root
  try {
    & $flutter pub get
    if ($LASTEXITCODE -ne 0) { throw "flutter pub get 실패" }
    & $flutter build windows --release
  } finally { Pop-Location }
}
$relDir = Join-Path $Root "build\windows\x64\runner\Release"
$exe = Join-Path $relDir "caffeine_desktop.exe"
if (-not (Test-Path $exe)) { throw "산출물이 없습니다: $exe" }
Write-Output "산출물: $exe"
if ($NoBundle) { exit 0 }

# 2. VC++ redist 스테이징
Invoke-Step "VC++ redist 스테이징" {
  powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $Root "tools\fetch_vcredist.ps1")
}

# 3. Inno Setup 인스톨러
$iss = Join-Path $Root "installer\installer.iss"
$iscc = @(
  (Get-Command ISCC -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue),
  (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
  "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
  "C:\Program Files\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw "ISCC.exe(Inno Setup 6+)를 찾지 못했습니다" }

$ver = "0.1.0"
$m = Select-String -LiteralPath (Join-Path $Root "pubspec.yaml") -Pattern '^version: ([0-9.]+)' | Select-Object -First 1
if ($m) { $ver = $m.Matches[0].Groups[1].Value }

Invoke-Step "Inno Setup 인스톨러 (v$ver)" {
  $out = & "$iscc" "$iss" "/DMyAppVersion=$ver" /Qp 2>&1 | ForEach-Object { "$_" }
  $code = $LASTEXITCODE
  if ($code -ne 0) {
    $out | ForEach-Object { Write-Output $_ }
    throw "ISCC 실패 (exit $code)"
  }
}
Write-Output "산출물: installer\Output\caffeine-desktop-setup-$ver.exe"
