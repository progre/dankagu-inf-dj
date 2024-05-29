use std::ffi::c_void;
use std::ffi::OsString;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;

use anyhow::anyhow;
use anyhow::Error;
use anyhow::Result;
use windows::core::Interface;
use windows::core::GUID;
use windows::Win32::Devices::HumanInterfaceDevice::DirectInput8Create;
use windows::Win32::Devices::HumanInterfaceDevice::IDirectInput8W;
use windows::Win32::Devices::HumanInterfaceDevice::IDirectInputDevice8W;
use windows::Win32::Devices::HumanInterfaceDevice::DI8DEVCLASS_GAMECTRL;
use windows::Win32::Devices::HumanInterfaceDevice::DIDATAFORMAT;
use windows::Win32::Devices::HumanInterfaceDevice::DIDEVCAPS;
use windows::Win32::Devices::HumanInterfaceDevice::DIDEVICEINSTANCEW;
use windows::Win32::Devices::HumanInterfaceDevice::DIEDFL_ALLDEVICES;
use windows::Win32::Devices::HumanInterfaceDevice::DIENUM_CONTINUE;
use windows::Win32::Devices::HumanInterfaceDevice::DIJOYSTATE2;
use windows::Win32::Devices::HumanInterfaceDevice::DIRECTINPUT_VERSION;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::WAIT_OBJECT_0;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateEventW;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::Win32::System::Threading::INFINITE;

#[allow(dead_code)]
extern "system" {
    pub static c_dfDIMouse: DIDATAFORMAT;
    pub static c_dfDIMouse2: DIDATAFORMAT;
    pub static c_dfDIKeyboard: DIDATAFORMAT;
    pub static c_dfDIJoystick: DIDATAFORMAT;
    pub static c_dfDIJoystick2: DIDATAFORMAT;
}

fn create_direct_input8() -> windows::core::Result<IDirectInput8W> {
    let handle = unsafe { GetModuleHandleW(None) }?;
    let mut iface: Option<IDirectInput8W> = None;
    unsafe {
        DirectInput8Create(
            handle,
            DIRECTINPUT_VERSION,
            &IDirectInput8W::IID,
            &mut iface as *mut _ as *mut *mut c_void,
            None,
        )
    }?;
    Ok(iface.unwrap())
}

#[derive(Debug)]
pub struct DeviceInstance {
    pub instance: GUID,
    pub product: GUID,
    pub dev_type: u32,
    pub instance_name: String,
    pub product_name: String,
    pub ff_driver: GUID,
    pub usage_page: u16,
    pub usage: u16,
}

impl From<&DIDEVICEINSTANCEW> for DeviceInstance {
    fn from(value: &DIDEVICEINSTANCEW) -> Self {
        let instance_name = {
            let end = value
                .tszInstanceName
                .iter()
                .position(|&ch| ch == '\0' as u16)
                .unwrap_or(value.tszInstanceName.len());
            OsString::from_wide(&value.tszInstanceName[..end])
                .to_string_lossy()
                .to_string()
        };
        let product_name = {
            let end = value
                .tszProductName
                .iter()
                .position(|&ch| ch == '\0' as u16)
                .unwrap_or(value.tszProductName.len());
            OsString::from_wide(&value.tszProductName[..end])
                .to_string_lossy()
                .to_string()
        };
        Self {
            instance: value.guidInstance,
            product: value.guidProduct,
            dev_type: value.dwDevType,
            instance_name,
            product_name,
            ff_driver: value.guidFFDriver,
            usage_page: value.wUsagePage,
            usage: value.wUsage,
        }
    }
}

fn device_instances(iface: &IDirectInput8W) -> windows::core::Result<Vec<DeviceInstance>> {
    let mut devices = Vec::new();
    extern "system" fn callback(ddi: *mut DIDEVICEINSTANCEW, devices: *mut c_void) -> BOOL {
        let devices = unsafe { (devices as *mut Vec<DeviceInstance>).as_mut() }.unwrap();
        devices.push(DeviceInstance::from(unsafe { ddi.as_ref() }.unwrap()));
        BOOL(DIENUM_CONTINUE as i32)
    }
    unsafe {
        iface.EnumDevices(
            DI8DEVCLASS_GAMECTRL,
            Some(callback),
            &mut devices as *mut _ as *mut c_void,
            DIEDFL_ALLDEVICES,
        )
    }?;
    Ok(devices)
}

fn _capabilities(device: &IDirectInputDevice8W) -> Result<DIDEVCAPS> {
    let mut caps = DIDEVCAPS {
        dwSize: size_of::<DIDEVCAPS>() as u32,
        ..Default::default()
    };
    unsafe { device.GetCapabilities(&mut caps) }?;
    Ok(caps)
}

pub fn create_device() -> IDirectInputDevice8W {
    let iface = create_direct_input8().unwrap();
    let device_instances = device_instances(&iface).unwrap();
    let device_inst = device_instances
        .iter()
        .find(|item| item.product_name == "PS3Controller")
        .expect("No device found");

    let mut device = None;
    unsafe { iface.CreateDevice(&device_inst.instance, &mut device, None) }.unwrap();
    let device = device.unwrap();

    unsafe { device.SetDataFormat(&c_dfDIJoystick2 as *const _ as *mut _) }.unwrap();
    device
}

pub fn init_event_notification(device: &IDirectInputDevice8W) -> HANDLE {
    let event = unsafe { CreateEventW(None, false, false, None) }.unwrap();
    unsafe { device.SetEventNotification(event) }.unwrap();
    event
}

pub fn acquire(device: &IDirectInputDevice8W) -> windows::core::Result<()> {
    unsafe { device.Acquire() }
}

pub fn get_state(device: &IDirectInputDevice8W, event: HANDLE) -> Result<DIJOYSTATE2> {
    if unsafe { WaitForSingleObject(event, INFINITE) } != WAIT_OBJECT_0 {
        return Err(unsafe { anyhow!("{:?}", GetLastError()) });
    }
    let mut state = DIJOYSTATE2::default();
    const SIZE: u32 = size_of::<DIJOYSTATE2>() as u32;
    unsafe { device.GetDeviceState(SIZE, &mut state as *mut _ as *mut c_void) }
        .map_err(|source| Error::new(source).context("Failed to get device state"))?;
    Ok(state)
}
