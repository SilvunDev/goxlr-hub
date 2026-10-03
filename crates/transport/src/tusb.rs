//! Windows: the device is reached through the library of the official
//! TC-Helicon driver, loaded at run time.

use std::ffi::{CStr, c_char};
use std::sync::OnceLock;
use std::thread::sleep;

use libloading::Library;
use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};

use crate::retry::{ReadError, read_when_ready};
use crate::{
    ACTIVATE_LEN, ANSWER_LEN, ATTEMPTS, Model, PAUSE, REQUEST_ACTIVATE, REQUEST_ANSWER,
    REQUEST_COMMAND, REQUEST_RESET, TransportError, best,
};

/// Where the driver registers its library. Read from the machine-wide part
/// of the registry only, so that a per-user entry cannot redirect the load.
const LIBRARY_KEY: &str =
    "SOFTWARE\\Classes\\CLSID\\{024D0372-641F-4B7B-8140-F4DFE458C982}\\InprocServer32";
const LIBRARY_FALLBACK: &str =
    "C:\\Program Files\\TC-HELICON\\GoXLR_Audio_Driver\\W10_x64\\goxlr_audioapi_x64.dll";

/// Every function of the library returns one of these; zero is success.
type Status = u32;
const STATUS_INVALID_HANDLE: Status = 0xee00_0048;

/// Request addressed to the interface, of the vendor kind.
const RECIPIENT_INTERFACE: u32 = 1;
const KIND_VENDOR: u32 = 0;

/// Room the library is given to report how many bytes went through.
const TRANSFERRED_LEN: usize = 64;

type EnumerateDevices = unsafe extern "C" fn() -> Status;
type GetDeviceCount = unsafe extern "C" fn() -> u32;
type OpenDeviceByIndex = unsafe extern "C" fn(u32, *mut u32) -> Status;
type GetDeviceProperties = unsafe extern "C" fn(u32, *mut Properties) -> Status;
type CloseDevice = unsafe extern "C" fn(u32) -> Status;
type StatusCodeString = unsafe extern "C" fn(Status) -> *const c_char;
#[rustfmt::skip]
type VendorRequestOut =
    unsafe extern "C" fn(u32, u32, u32, u32, u32, u16, u16, *const u8, *mut u8, u32) -> Status;
#[rustfmt::skip]
type VendorRequestIn =
    unsafe extern "C" fn(u32, u32, u32, u32, u32, u16, u16, *mut u8, *mut u8, u32) -> Status;

#[repr(C)]
struct Properties {
    vendor_id: i32,
    product_id: i32,
    revision: i32,
    serial_number: [u16; 128],
    manufacturer: [u16; 128],
    model: [u16; 128],
    reserved_number: i32,
    reserved_text: [u16; 128],
}

struct Api {
    enumerate_devices: EnumerateDevices,
    get_device_count: GetDeviceCount,
    open_device_by_index: OpenDeviceByIndex,
    get_device_properties: GetDeviceProperties,
    close_device: CloseDevice,
    status_code_string: StatusCodeString,
    vendor_request_out: VendorRequestOut,
    vendor_request_in: VendorRequestIn,
    /// Keeps the functions above loaded.
    _library: Library,
}

fn library_path() -> String {
    let key: Vec<u16> = LIBRARY_KEY.encode_utf16().chain([0]).collect();
    let mut path = [0u16; 520];
    let mut size = size_of_val(&path) as u32;
    // SAFETY: the key is null-terminated, and `size` is the size in bytes of
    // the buffer the value is written to.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            std::ptr::null(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            path.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return LIBRARY_FALLBACK.into();
    }
    let end = path
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(path.len());
    String::from_utf16_lossy(&path[..end])
}

impl Api {
    fn load() -> Result<Self, TransportError> {
        fn function<T: Copy>(library: &Library, name: &[u8]) -> Result<T, TransportError> {
            // SAFETY: each name is paired with the signature the driver
            // library exports it with.
            unsafe { library.get::<T>(name) }
                .map(|symbol| *symbol)
                .map_err(|error| TransportError::Io(error.to_string()))
        }

        // SAFETY: loads the library of the installed driver; its start-up
        // code has no requirement on the caller.
        let library = unsafe { Library::new(library_path()) }
            .map_err(|error| TransportError::Io(error.to_string()))?;
        Ok(Self {
            enumerate_devices: function(&library, b"TUSBAUDIO_EnumerateDevices\0")?,
            get_device_count: function(&library, b"TUSBAUDIO_GetDeviceCount\0")?,
            open_device_by_index: function(&library, b"TUSBAUDIO_OpenDeviceByIndex\0")?,
            get_device_properties: function(&library, b"TUSBAUDIO_GetDeviceProperties\0")?,
            close_device: function(&library, b"TUSBAUDIO_CloseDevice\0")?,
            status_code_string: function(&library, b"TUSBAUDIO_StatusCodeStringA\0")?,
            vendor_request_out: function(&library, b"TUSBAUDIO_ClassVendorRequestOut\0")?,
            vendor_request_in: function(&library, b"TUSBAUDIO_ClassVendorRequestIn\0")?,
            _library: library,
        })
    }

    /// The library, loaded on first use. A missing driver is tried again on
    /// the next call: it may be installed while the app runs.
    fn get() -> Result<&'static Self, TransportError> {
        static API: OnceLock<Api> = OnceLock::new();
        if let Some(api) = API.get() {
            return Ok(api);
        }
        let api = Self::load()?;
        Ok(API.get_or_init(|| api))
    }

    fn describe(&self, status: Status) -> String {
        // SAFETY: the library returns a pointer to a static, null-terminated
        // text for any status, or null.
        let text = unsafe { (self.status_code_string)(status) };
        if text.is_null() {
            return format!("driver status {status:#x}");
        }
        // SAFETY: checked non-null above.
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    }

    fn check(&self, status: Status) -> Result<(), TransportError> {
        if status == 0 {
            Ok(())
        } else {
            Err(TransportError::Io(self.describe(status)))
        }
    }

    fn open_index(&self, index: u32) -> Result<u32, TransportError> {
        let mut handle = 0;
        // SAFETY: `handle` outlives the call that writes it.
        self.check(unsafe { (self.open_device_by_index)(index, &mut handle) })?;
        Ok(handle)
    }

    fn close(&self, handle: u32) {
        // SAFETY: closing a handle, even a stale one, only returns a status.
        unsafe { (self.close_device)(handle) };
    }

    fn model(&self, handle: u32) -> Result<Option<Model>, TransportError> {
        // SAFETY: `Properties` is plain data, valid when zeroed, laid out as
        // the library fills it.
        let mut properties: Properties = unsafe { std::mem::zeroed() };
        // SAFETY: `properties` outlives the call that writes it.
        self.check(unsafe { (self.get_device_properties)(handle, &mut properties) })?;
        Ok(u16::try_from(properties.product_id)
            .ok()
            .and_then(Model::from_product_id))
    }

    /// The devices the driver sees, by index.
    fn devices(&self) -> Result<Vec<(u32, Model)>, TransportError> {
        // SAFETY: both calls take no argument and only refresh or read the
        // list kept by the library.
        let count = unsafe {
            (self.enumerate_devices)();
            (self.get_device_count)()
        };
        let mut devices = Vec::new();
        for index in 0..count {
            let handle = self.open_index(index)?;
            let model = self.model(handle);
            self.close(handle);
            if let Some(model) = model? {
                devices.push((index, model));
            }
        }
        Ok(devices)
    }
}

pub(crate) fn probe() -> Result<Option<Model>, TransportError> {
    // Without the driver, Windows shows no GoXLR to talk to.
    let Ok(api) = Api::get() else {
        return Ok(None);
    };
    Ok(best(api.devices()?.into_iter().map(|(_, model)| model)))
}

pub(crate) fn open() -> Result<UsbLink, TransportError> {
    let api = Api::get().map_err(|_| TransportError::NoDevice)?;
    let (index, _) = api
        .devices()?
        .into_iter()
        .find(|(_, model)| *model == Model::Full)
        .ok_or(TransportError::NoDevice)?;
    let mut link = UsbLink {
        api,
        handle: api.open_index(index)?,
    };

    // Wakes the vendor interface up, then starts a fresh session.
    link.read(REQUEST_ACTIVATE, ACTIVATE_LEN)
        .map_err(|status| TransportError::Io(api.describe(status)))?;
    link.write(REQUEST_RESET, &[])?;
    link.answer()?;
    Ok(link)
}

/// An open GoXLR.
pub struct UsbLink {
    api: &'static Api,
    handle: u32,
}

impl UsbLink {
    /// Sends one command and returns its answer.
    pub fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, TransportError> {
        self.write(REQUEST_COMMAND, request)?;
        self.answer()
    }

    fn answer(&mut self) -> Result<Vec<u8>, TransportError> {
        let api = self.api;
        read_when_ready(
            ATTEMPTS,
            || match self.read(REQUEST_ANSWER, ANSWER_LEN) {
                Ok(answer) if answer.is_empty() => Err(ReadError::NotReady("empty answer".into())),
                Ok(answer) => Ok(answer),
                Err(STATUS_INVALID_HANDLE) => {
                    Err(ReadError::Failed(api.describe(STATUS_INVALID_HANDLE)))
                }
                Err(status) => Err(ReadError::NotReady(api.describe(status))),
            },
            || sleep(PAUSE),
        )
    }

    fn write(&mut self, request: u8, data: &[u8]) -> Result<(), TransportError> {
        let length =
            u16::try_from(data.len()).map_err(|_| TransportError::Io("command too long".into()))?;
        let mut transferred = [0u8; TRANSFERRED_LEN];
        // SAFETY: `data` is `length` bytes long and `transferred` is as large
        // as announced; both outlive the call.
        let status = unsafe {
            (self.api.vendor_request_out)(
                self.handle,
                RECIPIENT_INTERFACE,
                KIND_VENDOR,
                request.into(),
                0,
                0,
                length,
                data.as_ptr(),
                transferred.as_mut_ptr(),
                TRANSFERRED_LEN as u32,
            )
        };
        self.api.check(status)
    }

    fn read(&mut self, request: u8, length: usize) -> Result<Vec<u8>, Status> {
        let mut buffer = vec![0u8; length];
        let mut transferred = [0u8; TRANSFERRED_LEN];
        // SAFETY: `buffer` is `length` bytes long and `transferred` is as
        // large as announced; both outlive the call.
        let status = unsafe {
            (self.api.vendor_request_in)(
                self.handle,
                RECIPIENT_INTERFACE,
                KIND_VENDOR,
                request.into(),
                0,
                0,
                length as u16,
                buffer.as_mut_ptr(),
                transferred.as_mut_ptr(),
                TRANSFERRED_LEN as u32,
            )
        };
        if status != 0 {
            return Err(status);
        }
        let received = u32::from_le_bytes([
            transferred[0],
            transferred[1],
            transferred[2],
            transferred[3],
        ]);
        buffer.truncate(received as usize);
        Ok(buffer)
    }
}

impl Drop for UsbLink {
    fn drop(&mut self) {
        self.api.close(self.handle);
    }
}
