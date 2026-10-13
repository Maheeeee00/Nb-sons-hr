; NB Sons HR Server - installer hooks
!macro NSIS_HOOK_PREINSTALL
  ; stop a running server so its files can be replaced
  nsExec::ExecToLog 'taskkill /F /IM hr-server.exe'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; let other PCs and phones on the network reach the HR System (port 5000)
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="NB Sons HR Server"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="NB Sons HR Server" dir=in action=allow protocol=TCP localport=5000 profile=any'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'taskkill /F /IM hr-server.exe'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="NB Sons HR Server"'
  ; NOTE: the data folder (Documents\NB Sons HR Data) is NOT deleted on uninstall.
!macroend
