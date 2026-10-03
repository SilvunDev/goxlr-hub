//! Linux and others: the device is reached through libusb.

use std::thread::sleep;
use std::time::Duration;

use rusb::{Device, DeviceHandle, Direction, GlobalContext, Recipient, RequestType};

use crate::retry::{ReadError, read_when_ready};
use crate::{
    ACTIVATE_LEN, ANSWER_LEN, ATTEMPTS, Model, PAUSE, REQUEST_ACTIVATE, REQUEST_ANSWER,
    REQUEST_COMMAND, REQUEST_RESET, TransportError, VENDOR_ID, best,
};

const TIMEOUT: Duration = Duration::from_secs(1);
/// The interface carrying the commands.
const INTERFACE: u8 = 0;
/// Lets the sound server let go of a device before it is claimed.
const SETTLE: Duration = Duration::from_millis(1500);

fn io(error: rusb::Error) -> TransportError {
    TransportError::Io(error.to_string())
}

fn devices() -> Result<Vec<(Device<GlobalContext>, Model)>, TransportError> {
    Ok(rusb::devices()
        .map_err(io)?
        .iter()
        .filter_map(|device| {
            let descriptor = device.device_descriptor().ok()?;
            if descriptor.vendor_id() != VENDOR_ID {
                return None;
            }
            Some((device, Model::from_product_id(descriptor.product_id())?))
        })
        .collect())
}

pub(crate) fn probe() -> Result<Option<Model>, TransportError> {
    Ok(best(devices()?.into_iter().map(|(_, model)| model)))
}

pub(crate) fn open() -> Result<UsbLink, TransportError> {
    let (device, _) = devices()?
        .into_iter()
        .find(|(_, model)| *model == Model::Full)
        .ok_or(TransportError::NoDevice)?;
    let handle = device.open().map_err(io)?;
    let claimed = handle.claim_interface(INTERFACE).is_ok();
    let mut link = UsbLink { handle };

    // Starts a fresh session. A device that was never set up since it was
    // powered refuses, and must be woken up first.
    match link.write(REQUEST_RESET, &[]) {
        Err(rusb::Error::Pipe) => link.wake_up(claimed).map_err(io)?,
        result => result.map_err(io)?,
    }
    link.answer()?;
    Ok(link)
}

/// An open GoXLR.
pub struct UsbLink {
    handle: DeviceHandle<GlobalContext>,
}

impl UsbLink {
    /// Sends one command and returns its answer.
    pub fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, TransportError> {
        self.write(REQUEST_COMMAND, request).map_err(io)?;
        sleep(PAUSE);
        self.answer()
    }

    fn answer(&mut self) -> Result<Vec<u8>, TransportError> {
        read_when_ready(
            ATTEMPTS,
            || match self.read(REQUEST_ANSWER, ANSWER_LEN) {
                Ok(answer) => Ok(answer),
                // The device stalls the read until the answer is there.
                Err(rusb::Error::Pipe) => Err(ReadError::NotReady(rusb::Error::Pipe.to_string())),
                Err(error) => Err(ReadError::Failed(error.to_string())),
            },
            || sleep(PAUSE),
        )
    }

    fn wake_up(&mut self, claimed: bool) -> Result<(), rusb::Error> {
        sleep(SETTLE);
        if claimed {
            self.handle.release_interface(INTERFACE)?;
        }
        self.handle.set_auto_detach_kernel_driver(true)?;
        self.handle.claim_interface(INTERFACE)?;

        self.read(REQUEST_ACTIVATE, ACTIVATE_LEN)?;
        // Turns the audio side on: sets the sample rate to 48 kHz.
        self.handle.write_control(
            rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface),
            1,
            0x0100,
            0x2900,
            &48_000u32.to_le_bytes(),
            TIMEOUT,
        )?;

        // Lets the system sound driver find the device again.
        self.handle.reset()?;
        self.handle.release_interface(INTERFACE)?;
        self.write(REQUEST_RESET, &[])
    }

    fn write(&mut self, request: u8, data: &[u8]) -> Result<(), rusb::Error> {
        self.handle.write_control(
            rusb::request_type(Direction::Out, RequestType::Vendor, Recipient::Interface),
            request,
            0,
            0,
            data,
            TIMEOUT,
        )?;
        Ok(())
    }

    fn read(&mut self, request: u8, length: usize) -> Result<Vec<u8>, rusb::Error> {
        let mut buffer = vec![0; length];
        let received = self.handle.read_control(
            rusb::request_type(Direction::In, RequestType::Vendor, Recipient::Interface),
            request,
            0,
            0,
            &mut buffer,
            TIMEOUT,
        )?;
        buffer.truncate(received);
        Ok(buffer)
    }
}
