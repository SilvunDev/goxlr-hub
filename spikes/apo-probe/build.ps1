# Builds the spike into dist/ and runs the unit test.
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { Write-Error 'MSVC build tools not found.'; exit 1 }
$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'

$flags = '/nologo /std:c++17 /W4 /WX /EHsc /O2 /MT /DUNICODE /D_UNICODE'
$steps = @(
    "cl $flags /Fo:dist\ /Fe:dist\gain_test.exe src\gain_test.cpp",
    'dist\gain_test.exe',
    "cl $flags /LD /Fo:dist\ /Fe:dist\goxlrhub_apo_probe.dll src\apo.cpp /link /DEF:src\apo.def /NODEFAULTLIB:atls.lib AudioBaseProcessingObjectV140.lib audiomediatypecrt.lib audioeng.lib ole32.lib uuid.lib",
    "cl $flags /Fo:dist\ /Fe:dist\probe.exe src\probe.cpp /link ole32.lib"
)

foreach ($step in $steps) {
    cmd /c "`"$vcvars`" >nul 2>nul && cd /d `"$root`" && $step"
    if ($LASTEXITCODE -ne 0) { Write-Host "Build step failed: $step"; exit 1 }
}
Write-Host 'build: ok'
