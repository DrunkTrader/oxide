#define AppVersion "0.1.0"
#ifndef PayloadDir
#define PayloadDir "..\..\target\release"
#endif

[Setup]
AppId={{5CB81644-D49D-40F3-88B4-2D4A39FA90E8}
AppName=Oxide
AppVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Oxide
DefaultGroupName=Oxide
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=Oxide-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\oxide-app.exe

[Files]
Source: "{#PayloadDir}\oxide-app.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxide-worker.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Oxide"; Filename: "{app}\oxide-app.exe"
Name: "{group}\Uninstall Oxide"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\oxide-app.exe"; Description: "Open Oxide"; Flags: nowait postinstall skipifsilent

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "Oxide"; Flags: uninsdeletevalue
