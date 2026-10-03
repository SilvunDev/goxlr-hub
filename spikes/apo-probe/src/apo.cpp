// Throwaway probe: a Windows audio effect (APO) that attenuates by 12 dB.
// It exists to prove that a module of ours can be loaded by the Windows audio
// engine on a GoXLR output. Written against the Windows SDK headers only.
#include <windows.h>

#include <audioenginebaseapo.h>
#include <baseaudioprocessingobject.h>

#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <new>

#include "gain.h"

// {24BE9B22-17F7-4083-A742-424A7B5B9CD9}
static const GUID CLSID_ProbeApo = {
    0x24BE9B22, 0x17F7, 0x4083, {0xA7, 0x42, 0x42, 0x4A, 0x7B, 0x5B, 0x9C, 0xD9}};

// One input, one output, same format on both sides, processed in place.
static const APO_REG_PROPERTIES g_regProps = {
    CLSID_ProbeApo,
    static_cast<APO_FLAG>(APO_FLAG_DEFAULT | APO_FLAG_INPLACE),
    L"GoXLR Hub probe effect",
    L"GPL-3.0-or-later",
    1, // major version
    0, // minor version
    1, // min input connections
    1, // max input connections
    1, // min output connections
    1, // max output connections
    ULONG_MAX, // max instances
    1, // interface count
    {__uuidof(IAudioProcessingObject)}};

static LONG g_objects = 0;
static LONG g_locks = 0;

// Never called from the real-time thread.
static void log_line(const char* format, ...) {
    wchar_t path[MAX_PATH];
    DWORD size = ExpandEnvironmentStringsW(L"%ProgramData%\\GoXLRHubSpike\\apo.log", path, MAX_PATH);
    if (size == 0 || size > MAX_PATH) {
        return;
    }

    char text[256];
    SYSTEMTIME now;
    GetLocalTime(&now);
    int head = sprintf_s(text, "%02u:%02u:%02u pid=%lu ", now.wHour, now.wMinute, now.wSecond,
                         GetCurrentProcessId());
    va_list args;
    va_start(args, format);
    _vsnprintf_s(text + head, sizeof(text) - head - 1, _TRUNCATE, format, args);
    va_end(args);
    size_t length = strlen(text);
    text[length++] = '\n';

    HANDLE file = CreateFileW(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr,
                              OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        return;
    }
    DWORD written = 0;
    WriteFile(file, text, static_cast<DWORD>(length), &written, nullptr);
    CloseHandle(file);
}

// Same layout as IUnknown. Lets the object keep its own reference count when
// the audio engine aggregates it.
struct INonDelegatingUnknown {
    virtual HRESULT STDMETHODCALLTYPE NonDelegatingQueryInterface(REFIID riid, void** object) = 0;
    virtual ULONG STDMETHODCALLTYPE NonDelegatingAddRef() = 0;
    virtual ULONG STDMETHODCALLTYPE NonDelegatingRelease() = 0;
};

class ProbeApo final : public CBaseAudioProcessingObject,
                       public IAudioSystemEffects,
                       public INonDelegatingUnknown {
public:
    explicit ProbeApo(IUnknown* outer)
        : CBaseAudioProcessingObject(&g_regProps),
          m_references(1),
          m_outer(outer != nullptr
                      ? outer
                      : reinterpret_cast<IUnknown*>(static_cast<INonDelegatingUnknown*>(this))) {
        InterlockedIncrement(&g_objects);
    }

    ~ProbeApo() { InterlockedDecrement(&g_objects); }

    // IUnknown, delegated to the outer object when aggregated.
    STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
        return m_outer->QueryInterface(riid, object);
    }
    STDMETHODIMP_(ULONG) AddRef() override { return m_outer->AddRef(); }
    STDMETHODIMP_(ULONG) Release() override { return m_outer->Release(); }

    HRESULT STDMETHODCALLTYPE NonDelegatingQueryInterface(REFIID riid, void** object) override {
        if (object == nullptr) {
            return E_POINTER;
        }
        if (riid == __uuidof(IUnknown)) {
            *object = static_cast<INonDelegatingUnknown*>(this);
            NonDelegatingAddRef();
            return S_OK;
        }
        if (riid == __uuidof(IAudioSystemEffects)) {
            *object = static_cast<IAudioSystemEffects*>(this);
        } else if (riid == __uuidof(IAudioProcessingObject)) {
            *object = static_cast<IAudioProcessingObject*>(this);
        } else if (riid == __uuidof(IAudioProcessingObjectRT)) {
            *object = static_cast<IAudioProcessingObjectRT*>(this);
        } else if (riid == __uuidof(IAudioProcessingObjectConfiguration)) {
            *object = static_cast<IAudioProcessingObjectConfiguration*>(this);
        } else {
            *object = nullptr;
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }

    ULONG STDMETHODCALLTYPE NonDelegatingAddRef() override {
        return InterlockedIncrement(&m_references);
    }

    ULONG STDMETHODCALLTYPE NonDelegatingRelease() override {
        LONG left = InterlockedDecrement(&m_references);
        if (left == 0) {
            delete this;
        }
        return left;
    }

    STDMETHODIMP Initialize(UINT32 dataSize, BYTE* data) override {
        HRESULT result = CBaseAudioProcessingObject::Initialize(dataSize, data);
        log_line("Initialize size=%u result=0x%08lX", dataSize, result);
        return result;
    }

    STDMETHODIMP LockForProcess(UINT32 inputCount, APO_CONNECTION_DESCRIPTOR** inputs,
                                UINT32 outputCount, APO_CONNECTION_DESCRIPTOR** outputs) override {
        HRESULT result =
            CBaseAudioProcessingObject::LockForProcess(inputCount, inputs, outputCount, outputs);
        if (SUCCEEDED(result)) {
            log_line("LockForProcess ok channels=%u rate=%.0f", GetSamplesPerFrame(),
                     GetFramesPerSecond());
        } else {
            log_line("LockForProcess result=0x%08lX", result);
        }
        return result;
    }

    // Real-time thread: no allocation, no lock, no logging.
    STDMETHODIMP_(void) APOProcess(UINT32, APO_CONNECTION_PROPERTY** inputs, UINT32,
                                   APO_CONNECTION_PROPERTY** outputs) override {
        const APO_CONNECTION_PROPERTY* input = inputs[0];
        APO_CONNECTION_PROPERTY* output = outputs[0];

        switch (input->u32BufferFlags) {
        case BUFFER_VALID:
            apply_gain(reinterpret_cast<float*>(output->pBuffer),
                       reinterpret_cast<const float*>(input->pBuffer),
                       static_cast<size_t>(input->u32ValidFrameCount) * GetSamplesPerFrame(),
                       PROBE_GAIN);
            output->u32ValidFrameCount = input->u32ValidFrameCount;
            output->u32BufferFlags = BUFFER_VALID;
            break;
        case BUFFER_SILENT:
            // Nothing to read: pass the silence on untouched.
            output->u32ValidFrameCount = input->u32ValidFrameCount;
            output->u32BufferFlags = BUFFER_SILENT;
            break;
        default:
            break;
        }
    }

private:
    LONG m_references;
    IUnknown* m_outer;
};

class ProbeFactory final : public IClassFactory {
public:
    STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
        if (object == nullptr) {
            return E_POINTER;
        }
        if (riid == __uuidof(IUnknown) || riid == __uuidof(IClassFactory)) {
            *object = static_cast<IClassFactory*>(this);
            return S_OK;
        }
        *object = nullptr;
        return E_NOINTERFACE;
    }

    // Static lifetime: reference counting is a no-op.
    STDMETHODIMP_(ULONG) AddRef() override { return 2; }
    STDMETHODIMP_(ULONG) Release() override { return 1; }

    STDMETHODIMP CreateInstance(IUnknown* outer, REFIID riid, void** object) override {
        if (object == nullptr) {
            return E_POINTER;
        }
        *object = nullptr;
        if (outer != nullptr && riid != __uuidof(IUnknown)) {
            return CLASS_E_NOAGGREGATION;
        }

        ProbeApo* apo = new (std::nothrow) ProbeApo(outer);
        if (apo == nullptr) {
            return E_OUTOFMEMORY;
        }
        HRESULT result = apo->NonDelegatingQueryInterface(riid, object);
        apo->NonDelegatingRelease();
        log_line("CreateInstance aggregated=%d result=0x%08lX", outer != nullptr, result);
        return result;
    }

    STDMETHODIMP LockServer(BOOL lock) override {
        if (lock) {
            InterlockedIncrement(&g_locks);
        } else {
            InterlockedDecrement(&g_locks);
        }
        return S_OK;
    }
};

static ProbeFactory g_factory;

STDAPI DllGetClassObject(REFCLSID clsid, REFIID riid, void** object) {
    if (clsid != CLSID_ProbeApo) {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    return g_factory.QueryInterface(riid, object);
}

STDAPI DllCanUnloadNow() {
    return (g_objects == 0 && g_locks == 0) ? S_OK : S_FALSE;
}
