; EposControl installer (Inno Setup 6). Build with scripts\build-installer.ps1.
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppId={{8C1D7A52-4B2E-4F7B-9A31-5E0C5C0A0001}
AppName=EposControl
AppVersion={#AppVersion}
AppPublisher=EposControl
DefaultDirName={autopf}\EposControl
DefaultGroupName=EposControl
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
OutputDir=..\dist
OutputBaseFilename=EposControl-Setup-{#AppVersion}
SetupIconFile=..\assets\eposcontrol.ico
LicenseFile=..\LICENSE
UninstallDisplayIcon={app}\eposcontrol.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=force

[Tasks]
Name: "startup"; Description: "Start EposControl hidden in the system tray when Windows starts"; GroupDescription: "Startup:"
Name: "replaceofficial"; Description: "Turn off the official EPOS Gaming Suite (its service, app and startup entry)"; GroupDescription: "Audio driver host:"
Name: "restartaudio"; Description: "Restart the Windows audio service now (briefly interrupts sound, needed once)"; GroupDescription: "Audio driver host:"
Name: "installdriver"; Description: "Install the EPOS audio driver (only needed if it is not installed yet)"; GroupDescription: "Audio driver host:"; Check: DriverMissing; Flags: unchecked
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "staging\eposcontrol.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
; Optional: official driver files, staged by build-installer.ps1 when -OfficialMsi is given.
Source: "staging\driver\*"; DestDir: "{app}\driver"; Flags: ignoreversion recursesubdirs skipifsourcedoesntexist; Tasks: installdriver

[Icons]
Name: "{group}\EposControl"; Filename: "{app}\eposcontrol.exe"
Name: "{group}\Uninstall EposControl"; Filename: "{uninstallexe}"
Name: "{autodesktop}\EposControl"; Filename: "{app}\eposcontrol.exe"; Tasks: desktopicon

[Registry]
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "EposControl"; ValueData: """{app}\eposcontrol.exe"" --minimized"; Flags: uninsdeletevalue; Tasks: startup

[Run]
; Driver, only when requested and the files were bundled.
Filename: "{sys}\pnputil.exe"; Parameters: "/add-driver ""{app}\driver\eposaudio.inf"" /install"; Flags: runhidden waituntilterminated; StatusMsg: "Installing the EPOS audio driver..."; Tasks: installdriver; Check: DriverFilesPresent
; Make way for our host service.
Filename: "{app}\eposcontrol.exe"; Parameters: "--disable-official"; Flags: runhidden waituntilterminated; Tasks: replaceofficial; StatusMsg: "Turning off the official EPOS Gaming Suite..."
; Our host service owns the shared memory the audio driver reads.
Filename: "{sys}\sc.exe"; Parameters: "create EposControlHost binPath= ""\""{app}\eposcontrol.exe\"" --service"" start= auto obj= ""NT AUTHORITY\LocalService"" DisplayName= ""EposControl Audio Host"""; Flags: runhidden waituntilterminated
Filename: "{sys}\sc.exe"; Parameters: "description EposControlHost ""Hosts the settings memory used by the EPOS audio driver for EposControl."""; Flags: runhidden waituntilterminated
Filename: "{sys}\sc.exe"; Parameters: "start EposControlHost"; Flags: runhidden waituntilterminated
Filename: "{sys}\net.exe"; Parameters: "stop audiosrv /y"; Flags: runhidden waituntilterminated; Tasks: restartaudio
Filename: "{sys}\net.exe"; Parameters: "start audiosrv"; Flags: runhidden waituntilterminated; Tasks: restartaudio
Filename: "{app}\eposcontrol.exe"; Description: "Open EposControl"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Give the official software back (service, startup shortcut and Run entries) if it was turned off.
Filename: "{app}\eposcontrol.exe"; Parameters: "--restore-official"; Flags: runhidden waituntilterminated; RunOnceId: "RestoreOfficial"
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM eposcontrol.exe"; Flags: runhidden; RunOnceId: "KillApp"
Filename: "{sys}\sc.exe"; Parameters: "stop EposControlHost"; Flags: runhidden waituntilterminated; RunOnceId: "StopHost"
Filename: "{sys}\sc.exe"; Parameters: "delete EposControlHost"; Flags: runhidden waituntilterminated; RunOnceId: "DeleteHost"

[Code]
function DriverMissing: Boolean;
begin
  Result := not RegKeyExists(HKCR, 'CLSID\{27201018-29C6-446A-9308-AA97260FEED1}');
end;

function DriverFilesPresent: Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\driver\eposaudio.inf'));
end;

procedure InitializeWizard;
begin
  if DriverMissing then
    MsgBox('The EPOS audio driver is not installed on this PC. EposControl needs it for the equalizer, surround and microphone effects.' + #13#10#13#10 +
           'Install the official EPOS Gaming Suite once, or tick "Install the EPOS audio driver" on the next page if this installer was built with the driver files.',
           mbInformation, MB_OK);
end;
