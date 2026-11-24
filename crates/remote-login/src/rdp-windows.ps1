$ErrorActionPreference = 'Stop'
$edition = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
if ($edition.EditionID -like 'Core*' -or $edition.InstallationType -eq 'Server Core') { exit 80 }
$user = Get-LocalUser -Name 'ANIMA_LOGIN_USER'
$group = Get-LocalGroup -SID 'S-1-5-32-555'
if (!(Get-LocalGroupMember $group | Where-Object { $_.SID -eq $user.SID })) {
    Add-LocalGroupMember -Group $group -Member $user
}
# Keep NLA and the existing firewall policy. Do not add a public-network exception.
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp' -Name UserAuthentication -Value 1
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 0
Start-Service TermService
