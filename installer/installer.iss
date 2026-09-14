; Caffeine Desktop — Inno Setup 6 installer script (Flutter build).
; Built by scripts\build.ps1 (ISCC.exe):
;   ISCC installer.iss /DMyAppVersion=0.2.1
; Requires: flutter build output at ..\build\windows\x64\runner\Release\
;           VC++ redist staged at staging\vc_redist.x64.exe (tools\fetch_vcredist.ps1)
#define MyAppName "Caffeine Desktop"
#define MyAppExe "caffeine_desktop.exe"
#ifndef MyAppVersion
  #define MyAppVersion "0.2.1"
#endif

[Setup]
AppId={{3B1F8E2A-7C4D-4E9F-A5B6-8D2C1E7F0A9B}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Caffeine Desktop
DefaultDirName={autopf}\Caffeine Desktop
DefaultGroupName={#MyAppName}
PrivilegesRequired=lowest
OutputDir={#SourcePath}\Output
OutputBaseFilename=caffeine-desktop-setup-{#MyAppVersion}
SetupIconFile=..\assets\icons\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64
DisableProgramGroupPage=yes
Compression=lzma2/max
SolidCompression=yes

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "startupicon"; Description: "Windows 시작 시 자동 실행 (백그라운드)"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; Flutter Windows output (exe + engine + plugin DLLs + flutter_assets).
Source: "..\build\windows\x64\runner\Release\*"; DestDir: "{app}"; Excludes: "*.pdb,*.exp,*.lib,*.ilk,*.map"; Flags: ignoreversion recursesubdirs createallsubdirs
; VC++ redist installer (downloaded at build time, removed after install).
Source: "staging\vc_redist.x64.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Parameters: "--background"; Tasks: startupicon

[Run]
Filename: "{tmp}\vc_redist.x64.exe"; Parameters: "/quiet /norestart"; StatusMsg: "Visual C++ 런타임 설치 중..."; Flags: skipifdoesntexist; Check: VCRedistNeedsInstall
Filename: "{app}\{#MyAppExe}"; Description: "지금 실행"; Flags: nowait postinstall skipifsilent

[Code]
// Standard VC++ 2015+ x64 redistributable detection (registry).
function VCRedistNeedsInstall(): Boolean;
var
  Version: Cardinal;
begin
  Result := True;
  if RegQueryDWordValue(
    HKLM64, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64',
    'Major', Version) then
  begin
    Result := (Version < 14);
  end
  else if RegQueryDWordValue(
    HKCU, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64',
    'Major', Version) then
  begin
    Result := (Version < 14);
  end;
end;
