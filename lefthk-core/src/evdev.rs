use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, BusType, Device, InputEvent, InputId, Key, RelativeAxisType};
use std::path::PathBuf;
use std::{collections::HashMap, ffi::OsStr};
use tokio::sync::{mpsc, oneshot};

use crate::errors::{self, LeftError};

#[derive(Debug)]
pub enum Task {
    KeyboardEvent((PathBuf, InputEvent)),
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

impl Default for EvDev {
    fn default() -> Self {
        let keys = AttributeSet::from_iter(
            (Key::KEY_RESERVED.code()..Key::BTN_TRIGGER_HAPPY40.code()).map(Key::new),
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

        let builder = errors::exit!(VirtualDeviceBuilder::new());

        let mut device = builder
            .name("LeftHK Virtual Keyboard")
            .input_id(InputId::new(BusType::BUS_I8042, 1, 1, 1))
            .with_keys(&keys)
            .unwrap()
            .with_relative_axes(&relative_axes)
            .unwrap()
            .build()
            .unwrap();
        println!("Device: {:?}", device.get_syspath());

        let devnode = device.enumerate_dev_nodes_blocking().unwrap().next();
        println!("Devnode: {:?}", devnode);

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
                    evdev.add_device(device);
                }
            }
            None => tracing::warn!("No devices found on intialization."),
        }

        evdev
    }
}

impl EvDev {
    pub fn add_device(&mut self, path: PathBuf) {
        tracing::info!("Adding device with path: {:?}", path);
        if let Ok(mut device) = Device::open(path.clone()) {
            wait_for_all_keys_unpressed(&device);
            errors::r#return!(device.grab());
            errors::r#return!(device.ungrab());
            errors::r#return!(device.grab());

            let (guard, task_guard) = oneshot::channel();
            let transmitter = self.task_transmitter.clone();

            let mut stream = errors::r#return!(device.into_event_stream());
            let p = path.clone();
            tokio::task::spawn(async move {
                while !guard.is_closed() {
                    match stream.next_event().await {
                        Ok(event) => {
                            transmitter
                                .send(Task::KeyboardEvent((p.clone(), event)))
                                .await
                                .unwrap();
                        }
                        Err(err) => {
                            tracing::warn!("Evdev device stream failed with {:?}", err);
                            // poll_fn(|cx| guard.poll_closed(cx)).await;
                            break;
                        }
                    }
                }
                tracing::info!("Device loop has closed.");
                errors::r#return!(stream.device_mut().ungrab());
            });

            self.task_guards.insert(path, task_guard);
        }
    }
    pub fn remove_device(&mut self, path: PathBuf) {
        tracing::info!("Removing device with path: {:?}", path);
        self.task_guards.remove(&path);
    }
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

fn wait_for_all_keys_unpressed(device: &Device) {
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
