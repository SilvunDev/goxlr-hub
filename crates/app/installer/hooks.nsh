; Additions of GoXLR Hub to the Windows installer made by Tauri.
;
; This file starts with a UTF-8 byte order mark and must keep it: without it
; NSIS reads the accented letters below in the code page of the build machine.

; Where the official TC-Helicon driver registers its library, and where it
; installs it. The app looks in the same places (crates/transport).
!define GOXLR_DRIVER_KEY "SOFTWARE\Classes\CLSID\{024D0372-641F-4B7B-8140-F4DFE458C982}\InprocServer32"
!define GOXLR_DRIVER_FILE "$PROGRAMFILES64\TC-HELICON\GoXLR_Audio_Driver\W10_x64\goxlr_audioapi_x64.dll"
!define GOXLR_DRIVER_HELP "https://github.com/SilvunDev/goxlr-hub#windows-driver"

; The words of the pages, in place of the stock ones of NSIS. This file is
; read before the pages are declared, so they pick these up. The
; strings are set right here, outside of any section as NSIS demands, so by
; language number: the names LANG_ENGLISH and LANG_FRENCH do not exist yet.
; English comes first, so that it stays the language by default.
!define MUI_WELCOMEPAGE_TEXT "$(goxlrHubWelcome)"
!define MUI_FINISHPAGE_TEXT "$(goxlrHubFinish)"
!define MUI_UNCONFIRMPAGE_TEXT_TOP "$(goxlrHubUninstall)"

!define GOXLR_HUB_ENGLISH 1033
!define GOXLR_HUB_FRENCH 1036
LangString goxlrHubWelcome ${GOXLR_HUB_ENGLISH} "Setup will install GoXLR Hub for your Windows account only. No administrator rights are needed.$\r$\n$\r$\nGoXLR Hub is an open source control app for the full-size TC-Helicon GoXLR. It is unofficial and not affiliated with TC-Helicon.$\r$\n$\r$\nIt works with the official TC-Helicon driver, which Setup neither installs nor changes.$\r$\n$\r$\n$_CLICK"
LangString goxlrHubWelcome ${GOXLR_HUB_FRENCH} "Ce programme va installer GoXLR Hub pour votre compte Windows uniquement. Aucun droit d'administrateur n'est demandé.$\r$\n$\r$\nGoXLR Hub est une application libre qui pilote la GoXLR de TC-Helicon (modèle complet). Elle est non officielle et n'est pas affiliée à TC-Helicon.$\r$\n$\r$\nElle fonctionne avec le driver officiel TC-Helicon, que ce programme n'installe pas et ne modifie pas.$\r$\n$\r$\n$_CLICK"

LangString goxlrHubFinish ${GOXLR_HUB_ENGLISH} "GoXLR Hub is installed for your Windows account. You will find it in the Start menu.$\r$\n$\r$\nClick Finish to close Setup."
LangString goxlrHubFinish ${GOXLR_HUB_FRENCH} "GoXLR Hub est installé pour votre compte Windows. Vous le trouverez dans le menu Démarrer.$\r$\n$\r$\nCliquez sur Fermer pour quitter ce programme."

LangString goxlrHubUninstall ${GOXLR_HUB_ENGLISH} "GoXLR Hub will be removed from your Windows account. The TC-Helicon driver stays in place. Your profiles are kept unless you tick the box below."
LangString goxlrHubUninstall ${GOXLR_HUB_FRENCH} "GoXLR Hub va être retiré de votre compte Windows. Le driver TC-Helicon reste en place. Vos profils sont gardés, sauf si vous cochez la case ci-dessous."

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
