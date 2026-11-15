; DND55 Windows installer (NSIS 3). Build:  makensis -DVERSION=0.2.0 -DSTAGE=<staging dir> -DOUTFILE=<path> installer.nsi
; The staging dir must contain DND55.exe and the docs listed below (scripts/build-windows-*.{sh,ps1} create it).

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef STAGE
  !define STAGE "..\..\dist\DND55"
!endif
!ifndef OUTFILE
  !define OUTFILE "..\..\dist\DND55-Setup.exe"
!endif

Name "DND55"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\DND55"
InstallDirRegKey HKLM "Software\DND55" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "DND55 ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "DND55"
VIAddVersionKey "FileDescription" "DND55 installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT OR Apache-2.0; includes SRD 5.2.1 (CC-BY-4.0)"

!define MUI_ICON "..\..\apps\dnd_desktop\assets\icon.ico"
!define MUI_UNICON "..\..\apps\dnd_desktop\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\DND55.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch DND55 now"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE}\LICENSE.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Italian"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "DND55 requires 64-bit Windows 10 or 11."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Section "DND55 (required)" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${STAGE}\DND55.exe"
  File "${STAGE}\README.txt"
  File "${STAGE}\LICENSE.txt"
  File "${STAGE}\WINDOWS_INSTALLATION_GUIDE.md"
  ; Saves live in %APPDATA%\DND55 (per user) so updates and uninstalls never touch them.
  SetShellVarContext current
  CreateDirectory "$APPDATA\DND55"
  SetShellVarContext all
  CreateDirectory "$SMPROGRAMS\DND55"
  CreateShortcut "$SMPROGRAMS\DND55\DND55.lnk" "$INSTDIR\DND55.exe" "" "$INSTDIR\DND55.exe" 0
  CreateShortcut "$SMPROGRAMS\DND55\Uninstall DND55.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\DND55" "InstallDir" "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55" "DisplayName" "DND55"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55" "Publisher" "DND55 project"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55" "DisplayIcon" "$INSTDIR\DND55.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55" "NoRepair" 1
SectionEnd

Section "Desktop shortcut" SecDesktop
  SetShellVarContext all
  CreateShortcut "$DESKTOP\DND55.lnk" "$INSTDIR\DND55.exe" "" "$INSTDIR\DND55.exe" 0
SectionEnd

Section "Allow hosting through Windows Firewall" SecFirewall
  nsExec::Exec 'netsh advfirewall firewall delete rule name="DND55"'
  nsExec::Exec 'netsh advfirewall firewall add rule name="DND55" dir=in action=allow program="$INSTDIR\DND55.exe" enable=yes profile=private,public'
SectionEnd

LangString DESC_Main ${LANG_ENGLISH} "The game itself."
LangString DESC_Desktop ${LANG_ENGLISH} "Put a DND55 icon on your desktop."
LangString DESC_Firewall ${LANG_ENGLISH} "Lets friends connect when you host a campaign (adds an inbound rule for DND55.exe only)."
LangString DESC_Main ${LANG_ITALIAN} "Il gioco."
LangString DESC_Desktop ${LANG_ITALIAN} "Crea un'icona di DND55 sul desktop."
LangString DESC_Firewall ${LANG_ITALIAN} "Permette agli amici di connettersi quando ospiti una campagna (regola in ingresso solo per DND55.exe)."
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} $(DESC_Main)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_Desktop)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecFirewall} $(DESC_Firewall)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function un.onInit
  SetRegView 64
FunctionEnd

Section "Uninstall"
  SetShellVarContext all
  Delete "$DESKTOP\DND55.lnk"
  RMDir /r "$SMPROGRAMS\DND55"
  Delete "$INSTDIR\DND55.exe"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\WINDOWS_INSTALLATION_GUIDE.md"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  nsExec::Exec 'netsh advfirewall firewall delete rule name="DND55"'
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\DND55"
  DeleteRegKey HKLM "Software\DND55"
  ; Saved campaigns and characters are kept unless the user explicitly asks to delete them.
  SetShellVarContext current
  IfSilent keep
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Also delete your saved campaigns, characters and settings?$\r$\n($APPDATA\DND55)$\r$\n$\r$\nChoose No to keep them for a future install." IDYES wipe IDNO keep
  wipe:
    RMDir /r "$APPDATA\DND55"
  keep:
SectionEnd
