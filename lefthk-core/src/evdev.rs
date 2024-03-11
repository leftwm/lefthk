use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, BusType, Device, InputEvent, InputId, Key, RelativeAxisType};
use std::path::PathBuf;
use std::{collections::HashMap, ffi::OsStr};
use tokio::sync::{mpsc, oneshot};

use crate::errors::{self, LeftError, Result};

#[derive(Debug)]
pub enum Task {
    KeyboardEvent(InputEvent),
    KeyboardAdded(PathBuf),
    KeyboardRemoved(PathBuf),
}

pub struct EvDev {
    pub device: VirtualDevice,
    pub task_receiver: mpsc::Receiver<Task>,
    task_transmitter: mpsc::Sender<Task>,
    task_guards: HashMap<PathBuf, oneshot::Receiver<()>>,
    _keyboard_watcher: KeyboardWatcher,
}

impl EvDev {
    pub fn new() -> Result<Self> {
        let device = generate_device().map_err(|_| LeftError::VirtualDeviceCreationFailed)?;

        let (task_transmitter, task_receiver) = mpsc::channel(128);

        let keyboard_watcher = KeyboardWatcher::new(task_transmitter.clone());

        let task_guards: HashMap<PathBuf, oneshot::Receiver<()>> = HashMap::new();

        let mut evdev = EvDev {
            device,
            task_receiver,
            task_transmitter,
            task_guards,
            _keyboard_watcher: keyboard_watcher,
        };

        match find_keyboards() {
            Some(devices) => {
                for device in devices {
                    errors::log!(evdev.add_device(device));
                }
            }
            None => tracing::warn!("No devices found on intialization."),
        }

        Ok(evdev)
    }

    pub fn add_device(&mut self, path: PathBuf) -> Result<()> {
        let p = path.clone();
        let path_str = p.to_str().ok_or(LeftError::PathToStrError)?;
        tracing::info!("Adding device with path: {:?}", path_str);
        let mut device = Device::open(path.clone())
            .map_err(|_| LeftError::DeviceOpenFailed(path_str.to_owned()))?;
        wait_for_keys_to_unpress(&device);
        device
            .grab()
            .map_err(|_| LeftError::DeviceGrabFailed(path_str.to_owned()))?;
        device
            .ungrab()
            .map_err(|_| LeftError::DeviceUngrabFailed(path_str.to_owned()))?;
        device
            .grab()
            .map_err(|_| LeftError::DeviceGrabFailed(path_str.to_owned()))?;

        let (guard, task_guard) = oneshot::channel();
        let transmitter = self.task_transmitter.clone();

        let mut stream = device.into_event_stream()?;
        tokio::task::spawn(async move {
            while !guard.is_closed() {
                match stream.next_event().await {
                    Ok(event) => {
                        transmitter.send(Task::KeyboardEvent(event)).await.unwrap();
                    }
                    Err(err) => {
                        tracing::warn!("Evdev device stream failed with {:?}", err);
                        break;
                    }
                }
            }
            tracing::info!("Closing loop for device {:?}.", p);
            errors::r#return!(stream.device_mut().ungrab());
        });

        self.task_guards.insert(path, task_guard);
        Ok(())
    }

    pub fn remove_device(&mut self, path: PathBuf) {
        tracing::info!("Removing device with path: {:?}", path);
        self.task_guards.remove(&path);
    }
}

fn generate_device() -> Result<VirtualDevice> {
    // Can't enable more keys due to wayland/kde
    let keys = AttributeSet::from_iter(
        (Key::KEY_RESERVED.code()..Key::KEY_ALS_TOGGLE.code()).map(Key::new),
    );

    let relative_axes = evdev::AttributeSet::from_iter([
        RelativeAxisType::REL_WHEEL,
        RelativeAxisType::REL_HWHEEL,
        RelativeAxisType::REL_X,
        RelativeAxisType::REL_Y,
        RelativeAxisType::REL_Z,
        RelativeAxisType::REL_RX,
        RelativeAxisType::REL_RY,
        RelativeAxisType::REL_RZ,
        RelativeAxisType::REL_DIAL,
        RelativeAxisType::REL_MISC,
        RelativeAxisType::REL_WHEEL_HI_RES,
        RelativeAxisType::REL_HWHEEL_HI_RES,
    ]);

    Ok(VirtualDeviceBuilder::new()?
        .name("LeftHK Virtual Keyboard")
        .input_id(InputId::new(BusType::BUS_USB, 0x1234, 0x5678, 0x111))
        .with_keys(&keys)?
        .with_relative_axes(&relative_axes)?
        .build()?)
}

fn find_keyboards() -> Option<Vec<PathBuf>> {
    let mut devices = vec![];
    let mut enumerator = udev::Enumerator::new().ok()?;
    enumerator.match_is_initialized().ok()?;
    enumerator.match_subsystem("input").ok()?;
    let enum_devices = enumerator.scan_devices().ok()?;
    for device in enum_devices {
        if let Some(devnode) = device.devnode() {
            let is_keyboard = device
                .property_value("ID_INPUT_KEYBOARD")
                .unwrap_or(OsStr::new("0"))
                == "1"
                && device
                    .property_value("ID_INPUT_MOUSE")
                    .unwrap_or(OsStr::new("0"))
                    == "0";
            if is_keyboard {
                devices.push(PathBuf::from(devnode));
            }
        }
    }
    Some(devices)
}

fn wait_for_keys_to_unpress(device: &Device) {
    let mut pending_release = false;
    loop {
        match device.get_key_state() {
            Ok(keys) if keys.iter().count() > 0 => pending_release = true,
            _ => break,
        }
    }
    if pending_release {
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
}

#[derive(Debug)]
struct KeyboardWatcher {
    _task_guard: oneshot::Receiver<()>,
}

impl KeyboardWatcher {
    pub fn new(task_transmitter: mpsc::Sender<Task>) -> Self {
        let (guard, task_guard) = oneshot::channel();

        tokio::task::spawn_blocking(move || {
            let mut socket = udev::MonitorBuilder::new()
                .expect("Failed to create monitor")
                .match_subsystem("input")
                .expect("Failed to match subsystem")
                .listen()
                .expect("Failed to listen");
            const SERVER: mio::Token = mio::Token(0);
            let mut poll = mio::Poll::new().expect("Failed to create poll");
            let mut events = mio::Events::with_capacity(1);
            poll.registry()
                .register(&mut socket, SERVER, mio::Interest::READABLE)
                .expect("Failed to register");
            while !guard.is_closed() {
                if let Err(err) = poll.poll(&mut events, None) {
                    tracing::warn!("KeyboardWatcher poll failed with {:?}", err);
                    continue;
                }

                for e in socket.iter() {
                    let device = e.device();
                    // for property in device.properties() {
                    //     tracing::info!("Property: {:?}, {:?}", property.name(), property.value());
                    // }
                    if device
                        .property_value("NAME")
                        .unwrap_or(OsStr::new(""))
                        .to_str()
                        .unwrap_or("")
                        .contains("LeftHK")
                    {
                        continue;
                    }
                    let is_keyboard = device
                        .property_value("ID_INPUT_KEYBOARD")
                        .unwrap_or(OsStr::new("0"))
                        == "1"
                        && device
                            .property_value("ID_INPUT_MOUSE")
                            .unwrap_or(OsStr::new("0"))
                            == "0";
                    if is_keyboard {
                        let path = device
                            .property_value("DEVNAME")
                            .unwrap_or(OsStr::new(""))
                            .to_owned();
                        if path.is_empty() {
                            continue;
                        }
                        match e.event_type() {
                            udev::EventType::Add => {
                                if let Err(err) = task_transmitter
                                    .try_send(Task::KeyboardAdded(PathBuf::from(path)))
                                {
                                    tracing::warn!(
                                        "Failed to send keyboard added event: {:?}",
                                        err
                                    );
                                }
                            }
                            udev::EventType::Remove => {
                                if let Err(err) = task_transmitter
                                    .try_send(Task::KeyboardRemoved(PathBuf::from(path)))
                                {
                                    tracing::warn!(
                                        "Failed to send keyboard removed event: {:?}",
                                        err
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
        Self {
            _task_guard: task_guard,
        }
    }
}
