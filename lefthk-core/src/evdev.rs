use evdev_rs::{Device, DeviceWrapper, InputEvent, ReadFlag, ReadStatus};
use std::future::poll_fn;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::{collections::HashMap, ffi::OsStr};
use tokio::sync::{mpsc, oneshot};

use crate::errors::{self, LeftError};

#[derive(Debug)]
pub enum Task {
    KeyboardEvent(InputEvent),
    KeyboardAdded(PathBuf),
    KeyboardRemoved(PathBuf),
}

pub struct EvDev {
    pub task_receiver: mpsc::Receiver<Task>,
    task_transmitter: mpsc::Sender<Task>,
    task_guards: HashMap<PathBuf, oneshot::Receiver<()>>,
    _keyboard_watcher: KeyboardWatcher,
}

impl Default for EvDev {
    fn default() -> Self {
        let (task_transmitter, task_receiver) = mpsc::channel(128);

        let keyboard_watcher = KeyboardWatcher::new(task_transmitter.clone());

        let task_guards: HashMap<PathBuf, oneshot::Receiver<()>> = HashMap::new();

        let devices = find_keyboards();

        let mut evdev = EvDev {
            task_receiver,
            task_transmitter,
            task_guards,
            _keyboard_watcher: keyboard_watcher,
        };
        match devices {
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
        if let Some(device) = device_with_path(path.clone()) {
            let (mut guard, task_guard) = oneshot::channel();
            let transmitter = self.task_transmitter.clone();
            const SERVER: mio::Token = mio::Token(0);
            let fd = device.file().as_raw_fd();
            let mut poll = errors::exit_on_error!(mio::Poll::new());
            let mut events = mio::Events::with_capacity(1);
            errors::exit_on_error!(poll.registry().register(
                &mut mio::unix::SourceFd(&fd),
                SERVER,
                mio::Interest::READABLE,
            ));

            tokio::task::spawn(async move {
                while !guard.is_closed() {
                    if let Err(err) = poll.poll(&mut events, None) {
                        tracing::warn!("Evdev device poll failed with {:?}", err);
                        continue;
                    }

                    while device.has_event_pending() {
                        match device.next_event(ReadFlag::NORMAL | ReadFlag::BLOCKING) {
                            Ok((ReadStatus::Success, event)) => {
                                transmitter.send(Task::KeyboardEvent(event)).await.unwrap();
                            }
                            Err(_) => {
                                poll_fn(|cx| guard.poll_closed(cx)).await;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                tracing::info!("Device loop has closed.");
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

fn device_with_path(path: PathBuf) -> Option<Device> {
    let device = Device::new_from_path(path).ok()?;
    if device.has(evdev_rs::enums::EventType::EV_KEY)
        && device.phys().unwrap_or("").contains("input0")
    {
        return Some(device);
    }
    None
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
