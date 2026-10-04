; Additions of GoXLR Hub to the Windows installer made by Tauri.

; Where the official TC-Helicon driver registers its library, and where it
; installs it. The app looks in the same places (crates/transport).
!define GOXLR_DRIVER_KEY "SOFTWARE\Classes\CLSID\{024D0372-641F-4B7B-8140-F4DFE458C982}\InprocServer32"
!define GOXLR_DRIVER_FILE "$PROGRAMFILES64\TC-HELICON\GoXLR_Audio_Driver\W10_x64\goxlr_audioapi_x64.dll"
!define GOXLR_DRIVER_HELP "https://github.com/SilvunDev/goxlr-hub#windows-driver"

; Tells when the driver is missing, and where to get it. Nothing is installed
; or changed: the driver stays the business of its maker.
!macro NSIS_HOOK_POSTINSTALL
  ; An update from the app, or an unattended install, asks nothing.
  ${If} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    SetRegView 64
    ReadRegStr $R8 HKLM "${GOXLR_DRIVER_KEY}" ""
    SetRegView lastused
    ${If} $R8 == ""
    ${AndIfNot} ${FileExists} "${GOXLR_DRIVER_FILE}"
      ; 1036 is French; every other language reads English.
      ${If} $LANGUAGE == 1036
        StrCpy $R9 "Le driver officiel TC-Helicon de la GoXLR n'a pas été trouvé sur ce PC.$\n$\nGoXLR Hub en a besoin pour dialoguer avec la GoXLR. Sans lui, l'appli s'ouvre en mode démonstration.$\n$\nOuvrir la page qui explique où le télécharger ?"
      ${Else}
        StrCpy $R9 "The official TC-Helicon driver of the GoXLR was not found on this computer.$\n$\nGoXLR Hub needs it to talk to the GoXLR. Without it, the app opens in demo mode.$\n$\nOpen the page that tells where to download it?"
      ${EndIf}
      MessageBox MB_YESNO|MB_ICONINFORMATION "$R9" IDNO goxlr_hub_driver_checked
        ExecShell "open" "${GOXLR_DRIVER_HELP}"
      goxlr_hub_driver_checked:
    ${EndIf}
  ${EndIf}
!macroend
