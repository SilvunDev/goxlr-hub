# Attaches the probe effect to one GoXLR output. Run as administrator, after build.ps1.
#
# Changes exactly one value of that output's FxProperties and keeps the original
# in a backup that uninstall.ps1 restores. It never writes any Windows security
# setting: DisableProtectedAudioDG is only read and reported.
param(
    [Parameter(Mandatory)][string]$Endpoint,
    [ValidateSet('Mfx', 'Sfx')][string]$Slot = 'Mfx'
)
$ErrorActionPreference = 'Stop'

$clsid = '{24BE9B22-17F7-4083-A742-424A7B5B9CD9}'
$installDir = Join-Path $env:ProgramFiles 'GoXLRHubSpike'
$dataDir = Join-Path $env:ProgramData 'GoXLRHubSpike'
$backup = Join-Path $dataDir 'backup.json'
$dll = Join-Path $PSScriptRoot 'dist\goxlrhub_apo_probe.dll'
# Mode effect (one instance per output) or stream effect (one per playing application).
$valueName = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},' + $(if ($Slot -eq 'Mfx') { 6 } else { 5 })

function Stop-WithMessage([string]$message) {
    Write-Host $message
    exit 1
}

$parsed = [guid]::Empty
if (-not [guid]::TryParse($Endpoint, [ref]$parsed)) { Stop-WithMessage 'The endpoint must be a GUID.' }
$device = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{$parsed}"
if (-not (Test-Path "$device\FxProperties")) { Stop-WithMessage 'No output with that id.' }
$interface = (Get-Item "$device\Properties").GetValue('{b3f8fa53-0004-438e-9003-51a46e139bfc},6')
if ($interface -notmatch 'GoXLR') { Stop-WithMessage "That output is not a GoXLR output ($interface)." }
if ((Get-Item $device).GetValue('DeviceState') -ne 1) { Stop-WithMessage 'That output is not active.' }
if (-not (Test-Path $dll)) { Stop-WithMessage 'Build first: run build.ps1.' }
$saved = if (Test-Path $backup) { Get-Content $backup -Raw | ConvertFrom-Json }
if ($saved -and ($saved.device -ne "{$parsed}" -or $saved.valueName -ne $valueName)) {
    Stop-WithMessage 'Already attached elsewhere. Run uninstall.ps1 first.'
}

$identity = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Stop-WithMessage 'Run this script as administrator.'
}

# 1. Backup, before anything is written. A second run keeps the first backup,
#    so the original value is never replaced by our own id.
if ($saved) {
    $original = $saved.original
} else {
    $original = (Get-Item "$device\FxProperties").GetValue($valueName)
    New-Item -ItemType Directory -Force $dataDir | Out-Null
    @{ device = "{$parsed}"; valueName = $valueName; original = $original } |
        ConvertTo-Json | Set-Content -Encoding utf8 $backup
}

# 2. The module itself, where the audio engine process can read it.
New-Item -ItemType Directory -Force $installDir | Out-Null
Copy-Item $dll $installDir -Force
$installed = Join-Path $installDir 'goxlrhub_apo_probe.dll'

# 3. COM class.
$server = "HKLM:\SOFTWARE\Classes\CLSID\$clsid\InprocServer32"
New-Item -Force $server | Out-Null
Set-ItemProperty $server -Name '(default)' -Value $installed
Set-ItemProperty $server -Name 'ThreadingModel' -Value 'Both'

# 4. Audio effect registration.
$apo = "HKLM:\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\$clsid"
New-Item -Force $apo | Out-Null
Set-ItemProperty $apo -Name 'FriendlyName' -Value 'GoXLR Hub probe effect'
Set-ItemProperty $apo -Name 'Copyright' -Value 'GPL-3.0-or-later'
Set-ItemProperty $apo -Name 'APOInterface0' -Value '{FD7F2B29-24D0-4B5C-B177-592C39F9CA10}'
$numbers = @{
    MajorVersion = 1; MinorVersion = 0; Flags = 15; NumAPOInterfaces = 1
    MinInputConnections = 1; MaxInputConnections = 1
    MinOutputConnections = 1; MaxOutputConnections = 1; MaxInstances = -1
}
foreach ($name in $numbers.Keys) {
    Set-ItemProperty $apo -Name $name -Value $numbers[$name] -Type DWord
}

# 5. Attach to the output. Administrators may only set values on this key, so
#    it is opened with exactly that right: asking for more is refused.
$fx = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey(
    "SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{$parsed}\FxProperties",
    [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree,
    [System.Security.AccessControl.RegistryRights]'QueryValues, SetValue')
$fx.SetValue($valueName, $clsid, [Microsoft.Win32.RegistryValueKind]::String)
$fx.Close()

# 6. The audio engine reads effects when it starts.
Restart-Service Audiosrv -Force

$protected = (Get-Item 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Audio').GetValue('DisableProtectedAudioDG')
Write-Host "Attached as $Slot to {$parsed}. Replaced: $original"
Write-Host "DisableProtectedAudioDG (read only): $(if ($null -eq $protected) { 'not set' } else { $protected })"
