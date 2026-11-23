$ErrorActionPreference = 'Stop'
$user = 'ANIMA_LOGIN_USER'
$existing = 'ANIMA_SSH_EXISTS'
$lock = New-Object Threading.Mutex($false, 'Global\AnimaPrivateSSHSetup')
if (!$lock.WaitOne(0)) { exit 81 }
try {
$root = Join-Path $env:ProgramData 'AnimaSSH'
if ((Test-Path $root) -and ((Get-Item $root).Attributes -band [IO.FileAttributes]::ReparsePoint)) { exit 82 }
New-Item -ItemType Directory -Force $root | Out-Null
$acl = New-Object Security.AccessControl.DirectorySecurity
$admins = New-Object Security.Principal.SecurityIdentifier 'S-1-5-32-544'
$system = New-Object Security.Principal.SecurityIdentifier 'S-1-5-18'
$acl.SetOwner($admins)
$acl.SetAccessRuleProtection($true, $false)
foreach ($sid in @($admins, $system)) {
    $rule = New-Object Security.AccessControl.FileSystemAccessRule($sid, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
    $acl.AddAccessRule($rule)
}
Set-Acl $root $acl
if ($existing -eq 'no') {
    $sshd = Join-Path $env:WINDIR 'System32\OpenSSH\sshd.exe'
    if (!(Test-Path $sshd)) {
        Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0 | Out-Null
    }
    if (!(Test-Path $sshd)) { exit 80 }
    $key = Join-Path $root 'host_ed25519'
    if (!(Test-Path $key)) {
        & "$env:WINDIR\System32\OpenSSH\ssh-keygen.exe" -q -t ed25519 -N '""' -f $key
        if ($LASTEXITCODE -ne 0) { exit 83 }
    }
    $config = Join-Path $root 'sshd_config'
    $keyPath = $key.Replace('\', '/')
    @"
Port 22
ListenAddress 127.0.0.1
HostKey "$keyPath"
PermitEmptyPasswords no
PasswordAuthentication yes
AllowUsers $($user.ToLowerInvariant())
Subsystem sftp sftp-server.exe
"@ | Set-Content -Encoding ascii $config
    & $sshd -t -f $config
    if ($LASTEXITCODE -ne 0) { exit 83 }
    $name = 'AnimaPrivateSSH'
    if (!(Get-Service -Name $name -ErrorAction SilentlyContinue)) {
        New-Service -Name $name -DisplayName 'Anima private SSH' -StartupType Automatic -BinaryPathName "`"$sshd`" -f `"$config`"" | Out-Null
    }
    Start-Service $name
}

} finally { $lock.ReleaseMutex(); $lock.Dispose() }
