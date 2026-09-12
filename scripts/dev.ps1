#Requires -Version 5.1
<#
.SYNOPSIS
  Caffeine Desktop 개발 실행 (flutter run, 콘솔 로그 표시).
#>
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if ((Test-Path $cargoBin) -and ($env:PATH -notlike "*$cargoBin*")) {
  $env:PATH = "$cargoBin;" + $env:PATH
}

$flutter = $null
if ($env:FLUTTER_ROOT -and (Test-Path (Join-Path $env:FLUTTER_ROOT "bin\flutter.bat"))) {
  $flutter = Join-Path $env:FLUTTER_ROOT "bin\flutter.bat"
} else {
  $f = Get-Command flutter -ErrorAction SilentlyContinue
  if ($f) { $flutter = $f.Source }
  elseif (Test-Path "C:\Develop\flutter\bin\flutter.bat") { $flutter = "C:\Develop\flutter\bin\flutter.bat" }
}
if (-not $flutter) { throw "Flutter SDK를 찾지 못했습니다 (FLUTTER_ROOT 설정 또는 설치 필요)" }

Push-Location $Root
try {
  & $flutter pub get
  if ($LASTEXITCODE -ne 0) { throw "flutter pub get 실패" }
  & $flutter run -d windows
} finally { Pop-Location }
