; Installer DevLock (NSIS). Dipasang per user di %LOCALAPPDATA%\DevLock tanpa hak admin;
; runtime, data database, dan project ikut di folder itu dan TIDAK dihapus saat uninstall.
;
; Build:  makensis /DVERSION=0.11.0 /DSRC=<folder berisi bin\, README.md, LICENSE-*> /DOUT=<setup.exe> devlock.nsi

Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !error "VERSION wajib diisi (/DVERSION=x.y.z)"
!endif
!ifndef SRC
  !error "SRC wajib diisi (/DSRC=folder paket rilis)"
!endif
!ifndef OUT
  !define OUT "devlock-v${VERSION}-windows-x64-setup.exe"
!endif

!define APP "DevLock"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\DevLock"

Name "${APP} ${VERSION}"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\DevLock"
InstallDirRegKey HKCU "Software\DevLock" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "${__FILEDIR__}\..\apps\desktop\src-tauri\icons\icon.ico"
!define MUI_UNICON "${__FILEDIR__}\..\apps\desktop\src-tauri\icons\icon.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\bin\devlock-gui.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Buka DevLock"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${SRC}\LICENSE-MIT"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Indonesian"
!insertmacro MUI_LANGUAGE "English"

Section "DevLock"
  ; Hentikan daemon versi lama agar binary bisa ditimpa (diabaikan bila belum terpasang).
  IfFileExists "$INSTDIR\bin\devlock.exe" 0 +2
    nsExec::Exec '"$INSTDIR\bin\devlock.exe" stop'

  SetOutPath "$INSTDIR\bin"
  File "${SRC}\bin\devlock.exe"
  File "${SRC}\bin\devlock-shim.exe"
  File "${SRC}\bin\devlock-gui.exe"
  SetOutPath "$INSTDIR"
  File "${SRC}\README.md"
  File "${SRC}\CHANGELOG.md"
  File "${SRC}\LICENSE-MIT"
  File "${SRC}\LICENSE-APACHE"

  ; Struktur folder, devlock.toml, dan shim.
  nsExec::Exec '"$INSTDIR\bin\devlock.exe" init'

  CreateDirectory "$SMPROGRAMS\DevLock"
  CreateShortcut "$SMPROGRAMS\DevLock\DevLock.lnk" "$INSTDIR\bin\devlock-gui.exe"
  CreateShortcut "$SMPROGRAMS\DevLock\Uninstall DevLock.lnk" "$INSTDIR\uninstall.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\DevLock" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "DevLock"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\bin\devlock-gui.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  ; Kembalikan perubahan sistem yang bisa dibatalkan tanpa UAC; hosts, DNS, dan CA komputer
  ; dijelaskan di README (`devlock hosts --clear`, `devlock dns disable`, `devlock ssl untrust`).
  nsExec::Exec '"$INSTDIR\bin\devlock.exe" stop'
  nsExec::Exec '"$INSTDIR\bin\devlock.exe" path remove'
  nsExec::Exec '"$INSTDIR\bin\devlock.exe" terminal remove'

  Delete "$INSTDIR\bin\devlock.exe"
  Delete "$INSTDIR\bin\devlock-shim.exe"
  Delete "$INSTDIR\bin\devlock-gui.exe"
  Delete "$INSTDIR\bin\*.old"
  RMDir "$INSTDIR\bin"
  RMDir /r "$INSTDIR\shims"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\CHANGELOG.md"
  Delete "$INSTDIR\LICENSE-MIT"
  Delete "$INSTDIR\LICENSE-APACHE"
  Delete "$INSTDIR\uninstall.exe"
  ; Runtime, database (data), project (www), dan devlock.toml dibiarkan: hapus manual bila
  ; memang tidak diperlukan lagi.
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\DevLock\DevLock.lnk"
  Delete "$SMPROGRAMS\DevLock\Uninstall DevLock.lnk"
  RMDir "$SMPROGRAMS\DevLock"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  DeleteRegKey HKCU "Software\DevLock"

  MessageBox MB_OK "DevLock dihapus. Folder $INSTDIR (runtime, data database, dan project di www) tidak dihapus; hapus manual bila sudah tidak diperlukan."
SectionEnd
