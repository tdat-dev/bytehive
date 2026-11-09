; ByteHive's Windows installer for the native app: bytehive.exe (the crate's
; bytehive-native.exe), its icon and the ByteHive for Chrome extension folder,
; installed for the current user.
;
; It takes the Tauri ByteHive's place: the same folder (%LOCALAPPDATA%\ByteHive)
; and the same uninstall key "bytehive", so the Tauri app's updater, which runs
; an installer with /UPDATE (or /P, passive), swaps the app in place and
; Add/Remove Programs still shows one ByteHive. The Tauri app wrote its agents
; and settings to ~/.bytehive/export/web-state.json (0.14.2 and later); the
; native app reads that file once on its first start.
;
; Build (tools/build_native_installer.ps1 does this):
;   makensis /DVERSION=0.15.0 /DEXE=<bytehive-native.exe> /DEXT=<browser-extension dir>
;            /DICON=<icon.ico> /DOUTFILE=<setup.exe> bytehive.nsi

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !error "pass /DVERSION=x.y.z"
!endif
!define APP "ByteHive"
!define KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\bytehive"
!define AUMID "com.bytehive.app"

Name "${APP}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\ByteHive"
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

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\bytehive.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open ByteHive"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; Never install into a source checkout (the key can point at one), and an
; updater's /P (passive) means no pages.
Function .onInit
  ; The key keeps the path in quotes.
  StrCpy $R1 $INSTDIR 1
  StrCmp $R1 '"' 0 +2
    StrCpy $INSTDIR $INSTDIR -1 1
  IfFileExists "$INSTDIR\.git\*.*" 0 +2
    StrCpy $INSTDIR "$LOCALAPPDATA\ByteHive"
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/P" $1
  IfErrors +2 0
    SetSilent silent
FunctionEnd

Section "ByteHive"
  ; The app being replaced (this one, or the Tauri ByteHive) lets go of its exe.
  nsExec::Exec 'taskkill /IM bytehive.exe /F'
  Sleep 800
  SetOutPath "$INSTDIR"
  File /oname=bytehive.exe "${EXE}"
  File /oname=icon.ico "${ICON}"
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

  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\bytehive.exe" "" "$INSTDIR\icon.ico"
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\bytehive.exe" "" "$INSTDIR\icon.ico"
SectionEnd

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
