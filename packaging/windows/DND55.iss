; Inno Setup 6 script — alternative to installer.nsi for building the installer natively on Windows.
; Run scripts\build-windows.ps1 first (creates dist\DND55\), then:  iscc packaging\windows\DND55.iss
#define AppVersion GetEnv("DND55_VERSION")
#if AppVersion == ""
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6B1C2A4E-55D5-4C55-9D55-0D5500000001}
AppName=DND55
AppVersion={#AppVersion}
AppPublisher=DND55 project
DefaultDirName={autopf}\DND55
DefaultGroupName=DND55
OutputDir=..\..\dist
OutputBaseFilename=DND55-Setup
SetupIconFile=..\..\apps\dnd_desktop\assets\icon.ico
UninstallDisplayIcon={app}\DND55.exe
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile=..\..\dist\DND55\LICENSE.txt
PrivilegesRequired=admin

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
Name: "firewall"; Description: "Allow hosting through Windows Firewall"

[Files]
Source: "..\..\dist\DND55\DND55.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\DND55\README.txt"; DestDir: "{app}"
Source: "..\..\dist\DND55\LICENSE.txt"; DestDir: "{app}"
Source: "..\..\dist\DND55\WINDOWS_INSTALLATION_GUIDE.md"; DestDir: "{app}"

[Dirs]
Name: "{userappdata}\DND55"; Flags: uninsneveruninstall

[Icons]
Name: "{group}\DND55"; Filename: "{app}\DND55.exe"
Name: "{group}\Uninstall DND55"; Filename: "{uninstallexe}"
Name: "{autodesktop}\DND55"; Filename: "{app}\DND55.exe"; Tasks: desktopicon

[Run]
Filename: "netsh"; Parameters: "advfirewall firewall add rule name=""DND55"" dir=in action=allow program=""{app}\DND55.exe"" enable=yes profile=private,public"; Flags: runhidden; Tasks: firewall
Filename: "{app}\DND55.exe"; Description: "Launch DND55"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""DND55"""; Flags: runhidden; RunOnceId: "fw"

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and not UninstallSilent then
    if MsgBox('Also delete your saved campaigns, characters and settings (' + ExpandConstant('{userappdata}\DND55') + ')?'#13#10'Choose No to keep them.', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      DelTree(ExpandConstant('{userappdata}\DND55'), True, True, True);
end;
