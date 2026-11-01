; Maestro's Windows installer: the native app (maestro.exe), its icon and the
; Maestro for Chrome extension folder, installed for the current user.
;
; It installs where the Tauri app was (its uninstall key "maestro" says
; where), under the same uninstall key, so the Tauri app's own updater moves
; people onto the native app and Add/Remove Programs shows one Maestro.
; It also registers the app's notification identity (com.maestro.app), so
; Windows notifications show "Maestro" and its icon.
;
; Build (tools/build_installer.ps1 does this):
;   makensis /DVERSION=0.11.0 /DEXE=<maestro.exe> /DEXT=<browser-extension dir>
;            /DOUTFILE=<setup.exe> maestro.nsi

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !error "pass /DVERSION=x.y.z"
!endif
!define APP "Maestro"
!define KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\maestro"
!define AUMID "com.maestro.app"

Name "${APP}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\maestro"
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
!define MUI_FINISHPAGE_RUN "$INSTDIR\maestro.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open Maestro"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; Never install into a source checkout (an old install key can point at one).
; The Tauri app's updater runs the installer with /P (passive): no pages.
Function .onInit
  IfFileExists "$INSTDIR\.git\*.*" 0 +2
    StrCpy $INSTDIR "$LOCALAPPDATA\maestro"
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/P" $1
  IfErrors +2 0
    SetSilent silent
FunctionEnd

Section "Maestro"
  ; The app being replaced (this one, or the Tauri one) has to let go of its exe.
  nsExec::Exec 'taskkill /IM maestro.exe /F'
  Sleep 800
  SetOutPath "$INSTDIR"
  File /oname=maestro.exe "${EXE}"
  File "..\assets\icons\icon.ico"
  SetOutPath "$INSTDIR\browser-extension"
  File /r /x .claude /x logs /x store "${EXT}\*.*"
  ; Marks the folder as ours, so uninstalling removes only what we put there.
  FileOpen $9 "$INSTDIR\browser-extension\.maestro-installed" w
  FileClose $9
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "${KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${KEY}" "Publisher" "tdat-dev"
  WriteRegStr HKCU "${KEY}" "MainBinaryName" "maestro.exe"
  WriteRegStr HKCU "${KEY}" "DisplayIcon" '"$INSTDIR\maestro.exe"'
  WriteRegStr HKCU "${KEY}" "InstallLocation" '"$INSTDIR"'
  WriteRegStr HKCU "${KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${KEY}" "EstimatedSize" $0

  WriteRegStr HKCU "Software\Classes\AppUserModelId\${AUMID}" "DisplayName" "${APP}"
  WriteRegStr HKCU "Software\Classes\AppUserModelId\${AUMID}" "IconUri" "$INSTDIR\icon.ico"

  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\maestro.exe" "" "$INSTDIR\icon.ico"
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\maestro.exe" "" "$INSTDIR\icon.ico"
SectionEnd

; Run by an updater (/S, or the Tauri updater's /UPDATE): open the app again.
Function .onInstSuccess
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/UPDATE" $1
  IfErrors 0 relaunch
  IfSilent relaunch done
  relaunch:
    Exec '"$INSTDIR\maestro.exe"'
  done:
FunctionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /IM maestro.exe /F'
  Sleep 800
  Delete "$INSTDIR\maestro.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\uninstall.exe"
  IfFileExists "$INSTDIR\browser-extension\.maestro-installed" 0 +2
    RMDir /r "$INSTDIR\browser-extension"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP}.lnk"
  Delete "$DESKTOP\${APP}.lnk"
  DeleteRegKey HKCU "${KEY}"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\${AUMID}"
SectionEnd
