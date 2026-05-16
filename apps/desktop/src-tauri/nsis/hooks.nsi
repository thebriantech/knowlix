!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$APPDATA\dev.knowlix.app"
  RMDir /r "$PROFILE\.knowlix"
!macroend
