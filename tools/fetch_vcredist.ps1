#Requires -Version 5.1
<#
.SYNOPSIS
  Visual C++ 재배포 패키지(x64)를 빌드 시점에 다운로드합니다 (Inno Setup 동봉용).
  출력: installer/staging/vc_redist.x64.exe
#>
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$OutDir = Join-Path $Root "installer\staging"
$Out = Join-Path $OutDir "vc_redist.x64.exe"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (Test-Path $Out) {
  Write-Output "vcredist already staged: $Out"
  exit 0
}
# Evergreen link for VS 2022 VC++ redistributable (x64).
$Url = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
Write-Output "downloading VC++ redist..."
Invoke-WebRequest -Uri $Url -OutFile $Out -UseBasicParsing
Write-Output "staged: $Out"
