use evdev_rs::{Device, DeviceWrapper, InputEvent, ReadFlag, ReadStatus};
use input::event::{DeviceEvent, EventTrait};
use input::{Event, Libinput, LibinputInterface};
use nix::libc::{O_RDWR, O_WRONLY};
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::errors::{self, LeftError};

#[derive(Debug)]
pub enum Task {
    KeyboardEvent(InputEvent),
    KeyboardAdded(String),
}

pub struct EvDev {
    pub task_receiver: mpsc::Receiver<Task>,
    task_transmitter: mpsc::Sender<Task>,
    task_guards: Vec<oneshot::Receiver<()>>,
    _keyboard_watcher: KeyboardWatcher,
}

impl Default for EvDev {
    fn default() -> Self {
        let (task_transmitter, task_receiver) = mpsc::channel(100);

        let keyboard_watcher = KeyboardWatcher::new(task_transmitter.clone());

        let task_guards: Vec<oneshot::Receiver<()>> = vec![];

        let devices = find_keyboards();

        let mut evdev = EvDev {
            task_receiver,
            task_transmitter,
            task_guards,
            _keyboard_watcher: keyboard_watcher,
        };
        for device in devices {
            evdev.add_device(device);
        }

        evdev
    }
}

impl EvDev {
    pub fn add_device(&mut self, path: PathBuf) {
        if let Some(device) = device_with_path(path) {
            let (guard, task_guard) = oneshot::channel();
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
            let timeout = Duration::from_millis(100);
            tokio::task::spawn(async move {
                loop {
                    if guard.is_closed() {
                        println!("Bye");
                        return;
                    }

                    if let Err(err) = poll.poll(&mut events, Some(timeout)) {
                        tracing::warn!("Evdev device poll failed with {:?}", err);
                        continue;
                    }

                    while device.has_event_pending() {
                        match device.next_event(ReadFlag::NORMAL | ReadFlag::BLOCKING) {
                            Ok((ReadStatus::Success, event)) => {
                                transmitter.send(Task::KeyboardEvent(event)).await.unwrap();
                            }
                            Err(_) => println!("Boo"),
                            _ => {}
                        }
                    }
                }
            });
            self.task_guards.push(task_guard);
        }
    }
}

struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read((flags != 0) | (flags & O_RDWR != 0))
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
            .open(path)
            .map(|file| file.into())
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: OwnedFd) {
        let _ = File::from(fd);
    }
}

fn find_keyboards() -> Vec<PathBuf> {
    let mut context = Libinput::new_with_udev(Interface);
    context.udev_assign_seat("seat0").unwrap();
    context.dispatch().unwrap();
    let mut devices = vec![];
    for event in &mut context {
        if let Event::Device(DeviceEvent::Added(_)) = &event {
            unsafe {
                if let Some(device) = event.device().udev_device() {
                    let is_keyboard = device
                        .property_value("ID_INPUT_KEYBOARD")
                        .unwrap_or(OsStr::new("0"))
                        == "1"
                        && device
                            .property_value("ID_INPUT_MOUSE")
                            .unwrap_or(OsStr::new("0"))
                            == "0";
                    if is_keyboard {
                        let path = device.property_value("DEVNAME").unwrap_or(OsStr::new(""));
                        devices.push(PathBuf::from(path))
                    }
                }
            }
        }
    }
    devices
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
    pub fn new(_task_transmitter: mpsc::Sender<Task>) -> Self {
        let (_guard, task_guard) = oneshot::channel();
        tokio::task::spawn(async move {});
        Self {
            _task_guard: task_guard,
        }
    }
}
