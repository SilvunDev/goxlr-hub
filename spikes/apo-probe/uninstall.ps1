# Removes the probe effect and puts the output back as install.ps1 found it.
# Run as administrator.
$ErrorActionPreference = 'Stop'

$clsid = '{24BE9B22-17F7-4083-A742-424A7B5B9CD9}'
$installDir = Join-Path $env:ProgramFiles 'GoXLRHubSpike'
$backup = Join-Path $env:ProgramData 'GoXLRHubSpike\backup.json'

if (-not (Test-Path $backup)) {
    Write-Host 'Nothing to remove: no backup found.'
    exit 1
}
$identity = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host 'Run this script as administrator.'
    exit 1
}

$saved = Get-Content $backup -Raw | ConvertFrom-Json
# Administrators may only set values on this key: open it with exactly that right.
$fx = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey(
    "SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\$($saved.device)\FxProperties",
    [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree,
    [System.Security.AccessControl.RegistryRights]'QueryValues, SetValue')
if ($null -eq $saved.original) {
    $fx.DeleteValue($saved.valueName, $false)
} else {
    $fx.SetValue($saved.valueName, $saved.original, [Microsoft.Win32.RegistryValueKind]::String)
}
$fx.Close()

Remove-Item "HKLM:\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\$clsid" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "HKLM:\SOFTWARE\Classes\CLSID\$clsid" -Recurse -Force -ErrorAction SilentlyContinue

# Restart first so the audio engine lets go of the module.
Restart-Service Audiosrv -Force
Remove-Item $installDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $backup -Force

Write-Host "Restored $($saved.valueName) on $($saved.device) to $($saved.original)."
