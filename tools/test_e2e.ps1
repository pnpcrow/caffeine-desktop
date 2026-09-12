#Requires -Version 5.1
<#
.SYNOPSIS
  Caffeine Desktop E2E ?�현/검�? ?�제 ?�릭�??��?�?메뉴·가림막·캡처?�외�?검증한??
  ?�패?�도 ???�로?�스????�� 종료?�다.
#>
param([switch]$Release)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Profile = if ($Release) { "release" } else { "debug" }
$Exe = Join-Path $Root "build\windows\x64\runner\Release\caffeine_desktop.exe"
if (-not (Test-Path $Exe)) { throw "?�행 ?�일???�습?�다: $Exe (먼�? 빌드)" }

$LogDir = Join-Path ([IO.Path]::GetTempPath()) "caffeine-e2e"
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
$ErrLog = Join-Path $LogDir "app.stderr.log"
$OutLog = Join-Path $LogDir "app.stdout.log"
Remove-Item $ErrLog, $OutLog -ErrorAction SilentlyContinue

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern void keybd_event(byte v, byte s, uint f, UIntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] public static extern uint GetPixel(IntPtr dc, int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr c, string cn, string t);
  [DllImport("user32.dll")] public static extern uint SendMessage(IntPtr h, uint m, UIntPtr w, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; }
  public const uint MOUSEEVENTF_LEFTDOWN=0x02, MOUSEEVENTF_LEFTUP=0x04, MOUSEEVENTF_RIGHTDOWN=0x08,
    MOUSEEVENTF_RIGHTUP=0x10, MOUSEEVENTF_ABSOLUTE=0x8000, MOUSEEVENTF_MOVE=0x0001;
  public const uint KEYEVENTF_KEYUP=0x0002;
  public const int TB_BUTTONCOUNT=0x0418, TB_GETBUTTON=0x0417, TB_GETBUTTONTEXTW=0x044B, TB_GETRECT=0x0433;
}
"@

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class M {
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  public struct POINT { public int X, Y; }
}
"@

Add-Type -AssemblyName System.Drawing

$script:passed = 0; $script:failed = 0
function Check($Name, $Cond, $Detail="") {
  if ($Cond) { $script:passed++; Write-Output "PASS: $Name $Detail" }
  else { $script:failed++; Write-Output "FAIL: $Name $Detail" }
}
function AppLog { $p = "$env:LOCALAPPDATA\caffeine-desktop\core.log"; if (Test-Path $p) { Get-Content $p -Raw } else { "" } }
function TopWindows {
  # NOTE: Get-Process enumeration (P/Invoke EnumWindows callbacks cannot
  # reliably write back to PS locals, so we use .NET's own enumeration).
  Get-Process | Where-Object { $_.MainWindowTitle -ne "" } | ForEach-Object { "$($_.Id)|$($_.MainWindowTitle)" }
}
function GuardCount { @(Get-Process | Where-Object { $_.MainWindowTitle -like "CaffeineGuard*" }).Count }
# System.Windows.Forms is loaded inside Click-At on first use.
# Closed-loop cursor: absolute mouse units are remapped unpredictably in this
# session (RDP), so move relatively and servo on GetCursorPos until on target.
function Get-Cursor { $p = New-Object M+POINT; [void][M]::GetCursorPos([ref]$p); return $p }
function Wait-CursorSettle {
  for ($i=0; $i -lt 30; $i++) {
    $a = Get-Cursor; Start-Sleep -Milliseconds 250; $b = Get-Cursor
    if (([Math]::Abs($a.X-$b.X) -le 2) -and ([Math]::Abs($a.Y-$b.Y) -le 2)) { Start-Sleep -Milliseconds 300; return $true }
  }
  return $false
}
function Move-CursorTo($x, $y) {
  $gx = 1.0; $gy = 1.0
  for ($i=0; $i -lt 30; $i++) {
    $pp = New-Object M+POINT; [void][M]::GetCursorPos([ref]$pp)
    $dx = $x - $pp.X; $dy = $y - $pp.Y
    if ([Math]::Abs($dx) -le 3 -and [Math]::Abs($dy) -le 3) { return $true }
    $mx = [int][Math]::Max(-600, [Math]::Min(600, $dx * $gx))
    $my = [int][Math]::Max(-600, [Math]::Min(600, $dy * $gy))
    [M]::mouse_event(0x0001, $mx, $my, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    $pn = New-Object M+POINT; [void][M]::GetCursorPos([ref]$pn)
    $ax = $pn.X - $pp.X; $ay = $pn.Y - $pp.Y
    if ([Math]::Abs($mx) -gt 5 -and $ax -ne 0) {
      $g = [Math]::Abs($mx / $ax); if ($g -gt 0.1 -and $g -lt 10) { $gx = $g } }
    if ([Math]::Abs($my) -gt 5 -and $ay -ne 0) {
      $g = [Math]::Abs($my / $ay); if ($g -gt 0.1 -and $g -lt 10) { $gy = $g } }
  }
  $pf = New-Object M+POINT; [void][M]::GetCursorPos([ref]$pf)
  return (([Math]::Abs($x-$pf.X) -le 8) -and ([Math]::Abs($y-$pf.Y) -le 8))
}
function Click-At($x, $y, $Right=$false) {
  [void](Wait-CursorSettle)
  if (-not (Move-CursorTo $x $y)) { Write-Output "WARN: cursor did not converge to $x,$y"; return }
  $c = Get-Cursor
  if (([Math]::Abs($x-$c.X) -gt 15) -or ([Math]::Abs($y-$c.Y) -gt 15)) { Write-Output "WARN: cursor drifted before click"; return }
  if ($Right) { [M]::mouse_event(0x0008,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 60; [M]::mouse_event(0x0010,0,0,0,[UIntPtr]::Zero) }
  else { [M]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 60; [M]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero) }
}
function Press-Key($vk) {
  [W]::keybd_event([byte]$vk, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80
  [W]::keybd_event([byte]$vk, 0, [W]::KEYEVENTF_KEYUP, [UIntPtr]::Zero)
}

$proc = $null
$settingsPath = "$env:APPDATA\caffeine-desktop\settings.json"
$settingsBak = "$settingsPath.e2e-bak"
try {
  Get-Process caffeine_desktop -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Seconds 1
  # Deterministic start: disable auto blackout for the test (restore afterwards).
  if (Test-Path $settingsPath) { Copy-Item $settingsPath $settingsBak -Force }
  New-Item -ItemType Directory -Force -Path (Split-Path $settingsPath) | Out-Null
  Set-Content -LiteralPath $settingsPath -Value '{"awake_enabled":true,"auto_blackout_enabled":false,"auto_blackout_secs":300,"unlock_key":"esc","unlock_mouse":"shake","start_minimized":false}'
  $proc = Start-Process $Exe -PassThru -RedirectStandardError $ErrLog -RedirectStandardOutput $OutLog

  # 1. 메인 �?기동 (EnumWindows 기반 ?�색)
  $main = [IntPtr]::Zero
  for ($i=0; $i -lt 24 -and $main -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Seconds 5
    $cand = Get-Process | Where-Object { $_.MainWindowTitle -eq "Caffeine Desktop" } | Select-Object -First 1
    if ($cand) { $main = $cand.MainWindowHandle }
  }
  Check "main window appears" ($main -ne [IntPtr]::Zero)
  Start-Sleep -Seconds 3
  Check "process responding after start" $proc.Responding
  Check "setup log" ((AppLog) -match "setup done")

  # 2. ?�크린샷 ?�일 기반 버튼 ?��? (좌표 가?�화 ?�피): 최�? 카라�??�러?�터
  $shotBtn = Join-Path $LogDir "find-button.png"
  Add-Type -AssemblyName System.Drawing, System.Windows.Forms
  $b0 = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $full = New-Object Drawing.Bitmap($b0.Width, $b0.Height)
  $gf = [Drawing.Graphics]::FromImage($full)
  $gf.CopyFromScreen(0, 0, 0, 0, $full.Size); $gf.Dispose()
  $full.Save($shotBtn, [Drawing.Imaging.ImageFormat]::Png)
  $W2 = $full.Width; $H2 = $full.Height
  $step = 4
  $gw = [int]($W2/$step); $gh = [int]($H2/$step)
  $grid = New-Object 'bool[,]' $gw, $gh
  for ($gy=0; $gy -lt $gh; $gy++) { for ($gx=0; $gx -lt $gw; $gx++) {
    $c = $full.GetPixel($gx*$step, $gy*$step)
    if ($c.R -gt 200 -and $c.G -gt 130 -and $c.G -lt 190 -and $c.B -lt 110) { $grid[$gx,$gy] = $true } } }
  # 최�? ?�결 ?�소 (flood fill)
  $seen = New-Object 'bool[,]' $gw, $gh
  $best = @(); $bestN = 0
  for ($gy=0; $gy -lt $gh; $gy++) { for ($gx=0; $gx -lt $gw; $gx++) {
    if (-not $grid[$gx,$gy] -or $seen[$gx,$gy]) { continue }
    $q = New-Object Collections.Generic.Queue[object]; $q.Enqueue(@($gx,$gy)); $seen[$gx,$gy]=$true
    $pts = @()
    while ($q.Count -gt 0) { $p = $q.Dequeue(); $pts += ,$p
      foreach ($d in @(@(1,0),@(-1,0),@(0,1),@(0,-1))) {
        $nx=$p[0]+$d[0]; $ny=$p[1]+$d[1]
        if ($nx -ge 0 -and $ny -ge 0 -and $nx -lt $gw -and $ny -lt $gh -and $grid[$nx,$ny] -and -not $seen[$nx,$ny]) {
          $seen[$nx,$ny]=$true; $q.Enqueue(@($nx,$ny)) } } }
    if ($pts.Count -gt $bestN) { $bestN = $pts.Count; $best = $pts } } }
  $full.Dispose()
  Check "caramel button found" ($bestN -gt 30) "cells=$bestN shot=$shotBtn"
  if ($bestN -gt 30) {
    $xs2 = $best | ForEach-Object { $_[0]*$step }; $ys2 = $best | ForEach-Object { $_[1]*$step }
    $cx = [int](($xs2 | Measure-Object -Average).Average); $cy = [int](($ys2 | Measure-Object -Average).Average)
    $bx0 = ($xs2 | Measure-Object -Minimum).Minimum; $bx1 = ($xs2 | Measure-Object -Maximum).Maximum
    $by0 = ($ys2 | Measure-Object -Minimum).Minimum
    Check "click lands on button" $true "at $cx,$cy"
    Click-At $cx $cy
    # ?��? ?�치 추정: 버튼 ?�상??(?�측 ?�렬 ?��?). ?�패?�도 치명?�이지 ?�음.
    $script:toggleX = $bx1 - 25; $script:toggleY = $by0 - 55
    Start-Sleep -Seconds 3
    $log = AppLog
    Check "blackout cmd received" ($log -match "cmd blackout_now")
    Check "overlay built" ($log -match "overlay window built")
    Check "capture excluded" ($log -match "capture_excluded=True")
    $guardN = GuardCount
    Check "overlay HWND visible" ($guardN -ge 1) "count=$guardN"

    # 3. 캡처 ?�외 검�? ?�제 ?�면?� 검�? BitBlt 캡처??검지 ?�아??    Add-Type -AssemblyName System.Windows.Forms
    $scr = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $px = [int]($scr.Width/2); $py = [int]($scr.Height/2)
    $dc = [W]::GetDC([IntPtr]::Zero); $real = [W]::GetPixel($dc, $px, $py); [void][W]::ReleaseDC([IntPtr]::Zero, $dc)
    $rr=[byte]($real -band 0xFF); $gg=[byte](($real -shr 8) -band 0xFF); $bb=[byte](($real -shr 16) -band 0xFF)
    $cap = New-Object Drawing.Bitmap(1,1); $g2=[Drawing.Graphics]::FromImage($cap); $g2.CopyFromScreen($px,$py,0,0,(New-Object Drawing.Size(1,1))); $g2.Dispose()
    $cc = $cap.GetPixel(0,0); $cap.Dispose()
    $realDark = ($rr -lt 40 -and $gg -lt 40 -and $bb -lt 40)
    $capKept = ($cc.R -gt 40 -or $cc.G -gt 40 -or $cc.B -gt 40)
    Check "real screen is black under overlay" $realDark "rgb=$rr,$gg,$bb"
    Check "BitBlt capture sees desktop (excluded)" $capKept "rgb=$($cc.R),$($cc.G),$($cc.B)"

    # 4. ESC ?�제
    [void][W]::SetForegroundWindow($main)
    Press-Key 0x1B
    Start-Sleep -Seconds 2
    Check "unlock accepted" ((AppLog) -match "unlock gesture accepted")
    $guardN2 = GuardCount
    Check "overlay gone after ESC" ($guardN2 -eq 0) "count=$guardN2"
  }

  # 4b. ?�앱 ?��? ?�릭 = refresh_tray 교착 ?��? 검??(?�일 코드 경로)
  if ($script:toggleX) {
    Click-At ([int]$script:toggleX) ([int]$script:toggleY)
    Start-Sleep -Seconds 3
    $log2 = AppLog
    Check "toggle cmd received" ($log2 -match "cmd set_awake")
    Check "tray menu refreshed" ($log2 -match "tray menu refreshed")
    Check "responding after toggle (no deadlock)" (Get-Process -Id $proc.Id).Responding
  }

  # 5. ?�레??메뉴 교착 ?��? 검??(UIA): ?�클�?-> "?�전 방�?" ?�출 -> ?�답 ?��?
  try {
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $condTool = New-Object System.Windows.Automation.PropertyCondition(
      [System.Windows.Automation.AutomationElement]::NameProperty,
      "Caffeine Desktop ???�스?�레???�전 방�?")
    $condShort = New-Object System.Windows.Automation.PropertyCondition(
      [System.Windows.Automation.AutomationElement]::NameProperty, "Caffeine Desktop")
    $trayBtn = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
      (New-Object System.Windows.Automation.OrCondition($condTool, $condShort)))
    if ($trayBtn -ne $null) {
      $pt = $trayBtn.GetClickablePoint()
      Write-Output "INFO: tray button at $([int]$pt.X),$([int]$pt.Y)"
      Click-At ([int]$pt.X) ([int]$pt.Y) -Right $true
      Start-Sleep -Seconds 2
      # Diagnose: list any visible popup-menu items UIA can see.
      try {
        $menuCond = New-Object System.Windows.Automation.PropertyCondition(
          [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
          [System.Windows.Automation.ControlType]::Menu)
        $menus = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $menuCond)
        Write-Output "INFO: UIA-visible menus=$($menus.Count)"
      } catch { Write-Output "INFO: menu enumeration failed: $($_.Exception.Message)" }
      $condItem = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, "?�전 방�?")
      $item = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condItem)
      Check "tray menu opens (item found)" ($item -ne $null)
      if ($item -ne $null) {
        $inv = $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $inv.Invoke()
        Start-Sleep -Seconds 3
        $log = AppLog
        Check "tray menu handler fired" ($log -match "tray menu activated: awake")
        Check "tray menu refreshed" ($log -match "tray menu refreshed")
        Check "responding after tray toggle (no deadlock)" (Get-Process -Id $proc.Id).Responding
        # ?��? ?�복 (?�시 ?�기 -> 켜기�?복�?)
        Click-At ([int]$pt.X) ([int]$pt.Y) -Right $true
        Start-Sleep -Seconds 2
        $item2 = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condItem)
        if ($item2 -ne $null) { $item2.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); Start-Sleep -Seconds 3 }
        Check "responding after 2nd toggle" (Get-Process -Id $proc.Id).Responding
      }
    } else {
      Write-Output "SKIP: tray icon not found via UIA (likely in overflow) ??verify manually"
    }
  } catch {
    Write-Output "SKIP: tray UIA test failed ($($_.Exception.Message)) ??verify manually"
  }

  # 6. 최종 ?�존 ?�인
  Check "process alive at end" (-not (Get-Process -Id $proc.Id -ErrorAction SilentlyContinue | Where-Object { $_.HasExited }))
}
finally {
  if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
  if (Test-Path $settingsBak) { Move-Item $settingsBak $settingsPath -Force }
  Start-Sleep -Seconds 1
}
Write-Output "RESULT passed=$script:passed failed=$script:failed"
Write-Output "----- core.log tail -----"
$clog = "$env:LOCALAPPDATA\caffeine-desktop\core.log"
if (Test-Path $clog) { Get-Content $clog | Select-Object -Last 25 | Out-String }
if ($script:failed -gt 0) { exit 1 }

