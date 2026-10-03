// Throwaway measurement tool for the audio effect spike.
//
//   probe list                 lists the active outputs
//   probe measure <id part>    plays a 1 kHz tone on one output and prints the
//                              peak level captured back from that same output
#include <windows.h>

#include <initguid.h>

// Order matters: the property key header needs the declarations from mmdeviceapi.
#include <mmdeviceapi.h>

#include <audioclient.h>
#include <functiondiscoverykeys_devpkey.h>
#include <mmreg.h>

#include <ks.h>
#include <ksmedia.h>

#include <cmath>
#include <cstdio>
#include <cwchar>
#include <cwctype>
#include <string>

template <class T>
struct Com {
    T* p = nullptr;
    ~Com() {
        if (p != nullptr) {
            p->Release();
        }
    }
    T* operator->() const { return p; }
    T** out() { return &p; }
};

static std::wstring lower(std::wstring text) {
    for (wchar_t& c : text) {
        c = static_cast<wchar_t>(towlower(c));
    }
    return text;
}

static std::wstring device_id(IMMDevice* device) {
    LPWSTR raw = nullptr;
    if (FAILED(device->GetId(&raw))) {
        return L"";
    }
    std::wstring id(raw);
    CoTaskMemFree(raw);
    return id;
}

static int list(IMMDeviceEnumerator* enumerator) {
    std::wstring defaultId;
    {
        Com<IMMDevice> device;
        if (SUCCEEDED(enumerator->GetDefaultAudioEndpoint(eRender, eConsole, device.out()))) {
            defaultId = device_id(device.p);
        }
    }

    Com<IMMDeviceCollection> devices;
    UINT count = 0;
    if (FAILED(enumerator->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, devices.out())) ||
        FAILED(devices->GetCount(&count))) {
        fprintf(stderr, "Cannot list the outputs.\n");
        return 1;
    }
    for (UINT i = 0; i < count; i++) {
        Com<IMMDevice> device;
        Com<IPropertyStore> store;
        if (FAILED(devices->Item(i, device.out())) ||
            FAILED(device->OpenPropertyStore(STGM_READ, store.out()))) {
            continue;
        }
        PROPVARIANT name;
        PropVariantInit(&name);
        store->GetValue(PKEY_Device_FriendlyName, &name);
        std::wstring id = device_id(device.p);
        wprintf(L"%c %s  %s\n", id == defaultId ? L'*' : L' ', id.c_str(),
                name.vt == VT_LPWSTR ? name.pwszVal : L"?");
        PropVariantClear(&name);
    }
    return 0;
}

static bool is_float32(const WAVEFORMATEX* format) {
    if (format->wBitsPerSample != 32) {
        return false;
    }
    if (format->wFormatTag == WAVE_FORMAT_IEEE_FLOAT) {
        return true;
    }
    return format->wFormatTag == WAVE_FORMAT_EXTENSIBLE &&
           reinterpret_cast<const WAVEFORMATEXTENSIBLE*>(format)->SubFormat ==
               KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
}

static int measure(IMMDeviceEnumerator* enumerator, const std::wstring& wanted) {
    Com<IMMDeviceCollection> devices;
    UINT count = 0;
    if (FAILED(enumerator->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, devices.out())) ||
        FAILED(devices->GetCount(&count))) {
        fprintf(stderr, "Cannot list the outputs.\n");
        return 1;
    }
    Com<IMMDevice> device;
    for (UINT i = 0; i < count && device.p == nullptr; i++) {
        Com<IMMDevice> candidate;
        if (SUCCEEDED(devices->Item(i, candidate.out())) &&
            lower(device_id(candidate.p)).find(lower(wanted)) != std::wstring::npos) {
            device.p = candidate.p;
            candidate.p = nullptr;
        }
    }
    if (device.p == nullptr) {
        fprintf(stderr, "No active output matches that id.\n");
        return 1;
    }

    Com<IAudioClient> render;
    Com<IAudioClient> loopback;
    WAVEFORMATEX* format = nullptr;
    if (FAILED(device->Activate(__uuidof(IAudioClient), CLSCTX_ALL, nullptr,
                                reinterpret_cast<void**>(render.out()))) ||
        FAILED(device->Activate(__uuidof(IAudioClient), CLSCTX_ALL, nullptr,
                                reinterpret_cast<void**>(loopback.out()))) ||
        FAILED(render->GetMixFormat(&format))) {
        fprintf(stderr, "Cannot open the output.\n");
        return 1;
    }
    if (!is_float32(format)) {
        fprintf(stderr, "The output does not mix in 32-bit float; cannot measure.\n");
        return 1;
    }

    const REFERENCE_TIME bufferTime = 2000000; // 200 ms
    Com<IAudioRenderClient> renderClient;
    Com<IAudioCaptureClient> captureClient;
    UINT32 bufferFrames = 0;
    HRESULT result = render->Initialize(AUDCLNT_SHAREMODE_SHARED, 0, bufferTime, 0, format, nullptr);
    if (SUCCEEDED(result)) {
        result = loopback->Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
                                      bufferTime, 0, format, nullptr);
    }
    if (SUCCEEDED(result)) {
        result = render->GetService(__uuidof(IAudioRenderClient),
                                    reinterpret_cast<void**>(renderClient.out()));
    }
    if (SUCCEEDED(result)) {
        result = loopback->GetService(__uuidof(IAudioCaptureClient),
                                      reinterpret_cast<void**>(captureClient.out()));
    }
    if (SUCCEEDED(result)) {
        result = render->GetBufferSize(&bufferFrames);
    }
    if (FAILED(result)) {
        fprintf(stderr, "Cannot start the streams (0x%08lX).\n", result);
        return 1;
    }

    const UINT32 channels = format->nChannels;
    const UINT32 rate = format->nSamplesPerSec;
    const double step = 2.0 * 3.14159265358979323846 * 1000.0 / rate;
    const UINT64 skipFrames = rate / 2; // let the streams settle
    const UINT64 wantedFrames = static_cast<UINT64>(rate) * 2;
    double phase = 0.0;
    UINT64 captured = 0;
    float peak = 0.0f;

    loopback->Start();
    render->Start();
    const ULONGLONG started = GetTickCount64();
    while (captured < wantedFrames && GetTickCount64() - started < 4000) {
        UINT32 padding = 0;
        if (SUCCEEDED(render->GetCurrentPadding(&padding)) && padding < bufferFrames) {
            const UINT32 frames = bufferFrames - padding;
            BYTE* data = nullptr;
            if (SUCCEEDED(renderClient->GetBuffer(frames, &data))) {
                float* sample = reinterpret_cast<float*>(data);
                for (UINT32 f = 0; f < frames; f++) {
                    const float value = static_cast<float>(0.5 * sin(phase));
                    phase += step;
                    for (UINT32 c = 0; c < channels; c++) {
                        *sample++ = value;
                    }
                }
                renderClient->ReleaseBuffer(frames, 0);
            }
        }

        UINT32 packet = 0;
        while (SUCCEEDED(captureClient->GetNextPacketSize(&packet)) && packet != 0) {
            BYTE* data = nullptr;
            UINT32 frames = 0;
            DWORD flags = 0;
            if (FAILED(captureClient->GetBuffer(&data, &frames, &flags, nullptr, nullptr))) {
                break;
            }
            if ((flags & AUDCLNT_BUFFERFLAGS_SILENT) == 0) {
                const float* sample = reinterpret_cast<const float*>(data);
                for (UINT32 f = 0; f < frames; f++) {
                    for (UINT32 c = 0; c < channels; c++, sample++) {
                        if (captured + f >= skipFrames && fabsf(*sample) > peak) {
                            peak = fabsf(*sample);
                        }
                    }
                }
            }
            captured += frames;
            captureClient->ReleaseBuffer(frames);
        }
        Sleep(10);
    }
    render->Stop();
    loopback->Stop();
    CoTaskMemFree(format);

    if (captured <= skipFrames) {
        fprintf(stderr, "Nothing was captured back from the output.\n");
        return 1;
    }
    printf("peak=%.4f frames=%llu rate=%u channels=%u\n", peak, captured, rate, channels);
    return 0;
}

int wmain(int argc, wchar_t** argv) {
    if (FAILED(CoInitializeEx(nullptr, COINIT_MULTITHREADED))) {
        return 1;
    }
    int code = 2;
    {
        Com<IMMDeviceEnumerator> enumerator;
        if (FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
                                    __uuidof(IMMDeviceEnumerator),
                                    reinterpret_cast<void**>(enumerator.out())))) {
            fprintf(stderr, "Cannot reach the Windows audio service.\n");
            code = 1;
        } else if (argc == 2 && wcscmp(argv[1], L"list") == 0) {
            code = list(enumerator.p);
        } else if (argc == 3 && wcscmp(argv[1], L"measure") == 0) {
            code = measure(enumerator.p, argv[2]);
        } else {
            fprintf(stderr, "Usage: probe list | probe measure <part of an output id>\n");
        }
    }
    CoUninitialize();
    return code;
}
