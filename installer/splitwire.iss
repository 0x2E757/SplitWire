; SplitWire-Setup-<version>.exe. Built by tools\release.ps1, which passes
; AppVersion, Source (the package folder), Icon and OutputDir.
#ifndef AppVersion
  #error Build with tools\release.ps1
#endif

[Setup]
AppId={{9E11B652-8C38-40BD-8E67-A554BAD616BF}
AppName=SplitWire
AppVersion={#AppVersion}
VersionInfoVersion={#AppVersion}
; SplitWire.exe install accepts only this folder, so a copy installed by hand is updated in place.
DefaultDirName={autopf}\SplitWire
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=admin
; wireguard.dll carries an amd64 kernel driver, which Arm64 Windows cannot load.
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
; The window's XAML Islands (WindowsXamlManager) need Windows 10 1903.
MinVersion=10.0.18362
OutputDir={#OutputDir}
OutputBaseFilename=SplitWire-Setup-{#AppVersion}
SetupIconFile={#Icon}
UninstallDisplayIcon={app}\SplitWire.Window.exe
UninstallDisplayName=SplitWire
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl,en.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl,ru.isl"

[Tasks]
; An update leaves the Run value as the window's switch last set it.
Name: "autostart"; Description: "{cm:TaskAutostart}"; Check: not IsUpdate

[Files]
Source: "{#Source}\SplitWire.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\SplitWire.Window.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\wireguard.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\wireguard-nt-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Start menu search finds the program by this shortcut.
Name: "{autoprograms}\SplitWire"; Filename: "{app}\SplitWire.Window.exe"

[Run]
; Registers the service or points it at this copy again, and starts it.
Filename: "{app}\SplitWire.exe"; Parameters: "{code:ServiceArguments}"; StatusMsg: "{cm:StartingService}"; Flags: runhidden waituntilterminated
; The same value the window's "Start SplitWire at sign-in" writes, in the hive of the user who started Setup.
Filename: "{sys}\reg.exe"; Parameters: "add HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v SplitWire /t REG_SZ /d ""\""{app}\SplitWire.Window.exe\"" --tray"" /f"; Tasks: autostart; Flags: runhidden waituntilterminated runasoriginaluser
Filename: "{app}\SplitWire.Window.exe"; Parameters: "--tray"; Flags: nowait runasoriginaluser
; A second start shows the window of the tray started above.
Filename: "{app}\SplitWire.Window.exe"; Description: "{cm:Launch}"; Flags: nowait postinstall runasoriginaluser skipifsilent

[Code]
const
  EVENT_MODIFY_STATE = $0002;
  SYNCHRONIZE = $00100000;
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';

var
  Updating: Boolean;

function OpenEvent(Access: DWORD; Inherit: BOOL; Name: String): THandle;
  external 'OpenEventW@kernel32.dll stdcall';
function OpenMutex(Access: DWORD; Inherit: BOOL; Name: String): THandle;
  external 'OpenMutexW@kernel32.dll stdcall';
function SetEvent(Target: THandle): BOOL;
  external 'SetEvent@kernel32.dll stdcall';
function CloseHandle(Target: THandle): BOOL;
  external 'CloseHandle@kernel32.dll stdcall';

{ The service is there for an update, also of a copy installed by hand. }
function InitializeSetup: Boolean;
begin
  Updating := RegKeyExists(HKLM, 'SYSTEM\CurrentControlSet\Services\SplitWire');
  Result := True;
end;

function IsUpdate: Boolean;
begin
  Result := Updating;
end;

{ --restore connects again the profiles the stop below disconnected, whatever their
  connect_at_boot; a fresh install connects only those with connect_at_boot. }
function ServiceArguments(Param: String): String;
begin
  if Updating then
    Result := 'install --restore'
  else
    Result := 'install';
end;

{ The window's Exit event ends it as the tray's Exit does; its mutex goes with the process. }
procedure CloseWindow;
var
  ExitEvent, Instance: THandle;
  Tries: Integer;
begin
  ExitEvent := OpenEvent(EVENT_MODIFY_STATE, False, 'Local\SplitWire.Window.Exit');
  if ExitEvent = 0 then
    Exit;
  SetEvent(ExitEvent);
  CloseHandle(ExitEvent);
  for Tries := 1 to 50 do begin
    Instance := OpenMutex(SYNCHRONIZE, False, 'Local\SplitWire.Window');
    if Instance = 0 then
      Break;
    CloseHandle(Instance);
    Sleep(100);
  end;
end;

{ The window and the service hold the files. Stopping the service disconnects every profile but
  keeps them in connected=; the profiles themselves stay untouched. }
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Result := '';
  CloseWindow;
  if Updating then
    Exec(ExpandConstant('{sys}\net.exe'), 'stop SplitWire', '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

{ uninstall stops and deletes the service, which disconnects everything, and removes the driver;
  cleanup removes NRPT rules a crashed service may have left. The data folder stays unless the
  user chooses otherwise. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Code: Integer;
  Arguments, Command, Service: String;
begin
  if CurUninstallStep = usUninstall then begin
    CloseWindow;
    Arguments := 'uninstall';
    if SuppressibleMsgBox(FmtMessage(CustomMessage('DeleteData'), [ExpandConstant('{commonappdata}\SplitWire')]),
         mbConfirmation, MB_YESNO or MB_DEFBUTTON2, IDNO) = IDYES then
      Arguments := 'uninstall --purge';
    Service := ExpandConstant('{app}\SplitWire.exe');
    if not Exec(Service, Arguments, '', SW_HIDE, ewWaitUntilTerminated, Code) or (Code <> 0) then
      SuppressibleMsgBox(FmtMessage(CustomMessage('UninstallFailed'), [IntToStr(Code)]), mbError, MB_OK, IDOK);
    Exec(Service, 'cleanup', '', SW_HIDE, ewWaitUntilTerminated, Code);
  end;
  { The Run value may also come from the window's switch; it goes when it starts this copy. }
  if (CurUninstallStep = usPostUninstall) and
     RegQueryStringValue(HKCU, RunKey, 'SplitWire', Command) and
     (Pos(Lowercase(ExpandConstant('{app}\')), Lowercase(Command)) > 0) then
    RegDeleteValue(HKCU, RunKey, 'SplitWire');
end;
