# apo-probe (throwaway spike)

Answers one question from stage 1: can an audio effect module written by us be
attached to a GoXLR output on Windows 11 while keeping the official driver?

**Verdict: yes.** Measured on Windows 11 24H2 (build 26100) with the official
TC-Helicon driver. No Windows security setting is needed: the unsigned module
loads with `DisableProtectedAudioDG` set to 0 as well as to 1.

## What is in here

- `src/apo.cpp`: the effect, a Windows audio processing object (APO) that
  attenuates by 12 dB. Written against the Windows SDK headers only.
- `src/probe.cpp`: plays a 1 kHz tone on one output and prints the peak level
  captured back from that same output.
- `install.ps1`, `uninstall.ps1`: attach the effect to one output and put the
  output back as it was.

## Run it

Needs Visual Studio Build Tools 2022 (C++), the Windows 11 SDK and an
administrator PowerShell for the two install scripts.

    .\build.ps1
    .\dist\probe.exe list
    .\dist\probe.exe measure <part of the output id>     # reference, 0.5000
    .\install.ps1 -Endpoint <output guid>                # as administrator
    .\dist\probe.exe measure <part of the output id>     # 0.1250 when the effect runs
    .\uninstall.ps1                                      # as administrator

The effect also writes what the audio engine asks of it to
`%ProgramData%\GoXLRHubSpike\apo.log`.

## Installation steps that worked

1. Copy the module to a folder the audio engine process can read
   (`%ProgramFiles%`).
2. Register the COM class: `HKLM\SOFTWARE\Classes\CLSID\{clsid}\InprocServer32`,
   default value = path of the module, `ThreadingModel` = `Both`.
3. Register the effect: `HKLM\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\{clsid}`
   with `FriendlyName`, `Copyright`, `MajorVersion`, `MinorVersion`, `Flags`,
   the four connection counts, `MaxInstances`, `NumAPOInterfaces` and
   `APOInterface0`.
4. Attach it to the output: in
   `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{output}\FxProperties`,
   set `{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},6` (mode effect) to the class id.
   Keep the previous value to restore it.
5. Restart the `Audiosrv` service.

Steps 1 to 5 need administrator rights and nothing else.

## What we learned

- Administrators hold only the "set value" right on `FxProperties`. The key has
  to be opened with exactly that right; asking for full write access is refused.
  No permission has to be changed.
- The audio engine creates the effect as an aggregated COM object, so the
  module must support aggregation.
- The SDK base class library asks the linker for the ATL library. It links and
  runs without it (`/NODEFAULTLIB:atls.lib`).
- Taking the mode effect slot replaces the Microsoft effect that was there, so
  the Windows "enhancements" of that output are gone while ours is attached.

## Not tested

- Listening by ear: the proof is the measurement and the effect's log.
- The stream effect slot (`install.ps1 -Slot Sfx`) and microphone inputs.
- A machine where Equalizer APO was never installed.
- Whether a Windows feature update or a driver reinstall resets the output's
  effects.

Fallback if this ever stops working: drive a separately installed Equalizer APO.

Nothing in this folder is reused as is by the application.
