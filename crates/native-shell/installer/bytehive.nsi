; ByteHive's Windows installer: the native app (bytehive.exe), its icon and the
; ByteHive for Chrome extension folder, installed for the current user.
;
; ByteHive used to be Maestro (a Tauri app). Installing takes Maestro's place:
; its exe, uninstaller, shortcuts and uninstall key "maestro" are removed (never
; anything in a git checkout), so Add/Remove Programs shows one app. The old
; app's updater runs this installer with /P (passive), so it updates people
; straight onto ByteHive. It registers the notification identity
; com.bytehive.app, so Windows notifications show "ByteHive" and its icon.
;
; Build (tools/build_installer.ps1 does this):
;   makensis /DVERSION=0.11.0 /DEXE=<native-shell.exe> /DEXT=<browser-extension dir>
;            /DOUTFILE=<setup.exe> bytehive.nsi

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !error "pass /DVERSION=x.y.z"
!endif
!define APP "ByteHive"
!define KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\bytehive"
!define AUMID "com.bytehive.app"
!define OLDKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\maestro"

Name "${APP}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\bytehive"
InstallDirRegKey HKCU "${KEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "${APP} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APP} installer"
VIAddVersionKey "LegalCopyright" "tdat-dev"

!define MUI_ICON "..\assets\icons\icon.ico"
!define MUI_UNICON "..\assets\icons\icon.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\bytehive.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open ByteHive"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; Never install into a source checkout (an old install key can point at one).
; The old Maestro's updater runs the installer with /P (passive): no pages.
Function .onInit
  IfFileExists "$INSTDIR\.git\*.*" 0 +2
    StrCpy $INSTDIR "$LOCALAPPDATA\bytehive"
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/P" $1
  IfErrors +2 0
    SetSilent silent
FunctionEnd

Section "ByteHive"
  ; The app being replaced (this one, or the old Maestro) has to let go of its exe.
  nsExec::Exec 'taskkill /IM bytehive.exe /F'
  nsExec::Exec 'taskkill /IM maestro.exe /F'
  Sleep 800
  SetOutPath "$INSTDIR"
  File /oname=bytehive.exe "${EXE}"
  File "..\assets\icons\icon.ico"
  SetOutPath "$INSTDIR\browser-extension"
  File /r /x .claude /x logs /x store "${EXT}\*.*"
  ; Marks the folder as ours, so uninstalling removes only what we put there.
  FileOpen $9 "$INSTDIR\browser-extension\.bytehive-installed" w
  FileClose $9
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "${KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${KEY}" "Publisher" "tdat-dev"
  WriteRegStr HKCU "${KEY}" "MainBinaryName" "bytehive.exe"
  WriteRegStr HKCU "${KEY}" "DisplayIcon" '"$INSTDIR\bytehive.exe"'
  WriteRegStr HKCU "${KEY}" "InstallLocation" '"$INSTDIR"'
  WriteRegStr HKCU "${KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${KEY}" "EstimatedSize" $0

  WriteRegStr HKCU "Software\Classes\AppUserModelId\${AUMID}" "DisplayName" "${APP}"
  WriteRegStr HKCU "Software\Classes\AppUserModelId\${AUMID}" "IconUri" "$INSTDIR\icon.ico"

  Call RemoveOldMaestro
  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\bytehive.exe" "" "$INSTDIR\icon.ico"
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\bytehive.exe" "" "$INSTDIR\icon.ico"
SectionEnd

; The old Maestro: its files where it was installed (unless that is a git
; checkout), its shortcuts, its uninstall key and notification identity.
Function RemoveOldMaestro
  ReadRegStr $R0 HKCU "${OLDKEY}" "InstallLocation"
  StrCmp $R0 "" keys
  ; The key keeps the path in quotes.
  StrCpy $R1 $R0 1
  StrCmp $R1 '"' 0 +2
    StrCpy $R0 $R0 -1 1
  StrCmp $R0 $INSTDIR keys
  IfFileExists "$R0\.git\*.*" keys
  IfFileExists "$R0\maestro.exe" 0 keys
    Delete "$R0\maestro.exe"
    Delete "$R0\uninstall.exe"
    RMDir /r "$R0\browser-extension"
    RMDir "$R0"
  keys:
  DeleteRegKey HKCU "${OLDKEY}"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\com.maestro.app"
  Delete "$SMPROGRAMS\maestro.lnk"
  Delete "$DESKTOP\maestro.lnk"
FunctionEnd

; Run by an updater (/S, or the Tauri updater's /UPDATE): open the app again.
Function .onInstSuccess
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/UPDATE" $1
  IfErrors 0 relaunch
  IfSilent relaunch done
  relaunch:
    Exec '"$INSTDIR\bytehive.exe"'
  done:
FunctionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /IM bytehive.exe /F'
  Sleep 800
  Delete "$INSTDIR\bytehive.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\uninstall.exe"
  IfFileExists "$INSTDIR\browser-extension\.bytehive-installed" 0 +2
    RMDir /r "$INSTDIR\browser-extension"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP}.lnk"
  Delete "$DESKTOP\${APP}.lnk"
  DeleteRegKey HKCU "${KEY}"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\${AUMID}"
SectionEnd
