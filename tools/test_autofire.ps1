#Requires -Version 5.1
<#
.SYNOPSIS
  ?�동 가�?발화 검�? ?�릭 ?�이 ?�휴 ?�?�머만으�??�버?�이+캡처?�외�?검증한??
  (?�성 ?�릭????먹는 ?�경?�서???�심 보장???�인?????�다.)
#>
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Exe = Join-Path $Root "build\windows\x64\runner\Release\caffeine_desktop.exe"
if (-not (Test-Path $Exe)) { throw "?�행 ?�일???�습?�다: $Exe" }

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class PX { [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h); [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc); [DllImport("gdi32.dll")] public static extern uint GetPixel(IntPtr dc, int x, int y); }
"@

$passed = 0; $failed = 0
function Check($N, $C, $D="") { if ($C) { $script:passed++; Write-Output "PASS: $N $D" } else { $script:failed++; Write-Output "FAIL: $N $D" } }

$settingsPath = "$env:APPDATA\caffeine-desktop\settings.json"
$settingsBak = "$settingsPath.autofire-bak"
$proc = $null
try {
  Get-Process caffeine_desktop -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Seconds 1
  if (Test-Path $settingsPath) { Copy-Item $settingsPath $settingsBak -Force }
  New-Item -ItemType Directory -Force -Path (Split-Path $settingsPath) | Out-Null
  # 5�??�?�아?? 기동 그레?�스(60s) ?�후 발화까�? �?~70s ?��?  Set-Content -LiteralPath $settingsPath -Value '{"awake_enabled":true,"auto_blackout_enabled":true,"auto_blackout_secs":5,"unlock_key":"esc","unlock_mouse":"off","start_minimized":false}'
  $proc = Start-Process $Exe -PassThru
  $fired = $false
  for ($i=0; $i -lt 20; $i++) {
    Start-Sleep -Seconds 5
    $n = @(Get-Process | Where-Object { $_.MainWindowTitle -like "CaffeineGuard*" }).Count
    if ($n -ge 1) { $fired = $true; break }
    if ((Get-Process -Id $proc.Id -ErrorAction SilentlyContinue) -eq $null) { break }
  }
  Check "auto blackout fired (no clicks needed)" $fired
  if ($fired) {
    Start-Sleep -Seconds 2
    $scr = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $px = [int]($scr.Width/2); $py = [int]($scr.Height/2)
    $dc = [PX]::GetDC([IntPtr]::Zero); $real = [PX]::GetPixel($dc, $px, $py); [void][PX]::ReleaseDC([IntPtr]::Zero, $dc)
    $rr=[byte]($real -band 0xFF); $gg=[byte](($real -shr 8) -band 0xFF); $bb=[byte](($real -shr 16) -band 0xFF)
    $cap = New-Object Drawing.Bitmap(1,1); $g2=[Drawing.Graphics]::FromImage($cap)
    $g2.CopyFromScreen($px,$py,0,0,(New-Object Drawing.Size(1,1))); $g2.Dispose()
    $cc = $cap.GetPixel(0,0); $cap.Dispose()
    Check "real screen is black under overlay" (($rr -lt 40 -and $gg -lt 40 -and $bb -lt 40)) "rgb=$rr,$gg,$bb"
    Check "BitBlt capture sees desktop (excluded)" (($cc.R -gt 40 -or $cc.G -gt 40 -or $cc.B -gt 40)) "rgb=$($cc.R),$($cc.G),$($cc.B)"
  }
  Check "process responding" (Get-Process -Id $proc.Id).Responding
}
finally {
  if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
  if (Test-Path $settingsBak) { Move-Item $settingsBak $settingsPath -Force }
  Start-Sleep -Seconds 1
}
Write-Output "RESULT passed=$passed failed=$failed"
if ($failed -gt 0) { exit 1 }

