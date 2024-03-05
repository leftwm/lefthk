use evdev_rs::{Device, DeviceWrapper, InputEvent, ReadFlag, ReadStatus};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::errors::{self, LeftError};

pub struct EvDev {
    pub task_receiver: mpsc::Receiver<InputEvent>,
    _task_guards: Vec<oneshot::Receiver<()>>,
}

impl EvDev {
    pub fn new() -> Self {
        let (tx, task_receiver) = mpsc::channel(100);

        let mut task_guards: Vec<oneshot::Receiver<()>> = vec![];
        for entry in errors::exit_on_error!(std::fs::read_dir("/dev/input")) {
            let entry = errors::exit_on_error!(entry);

            if !entry
                .file_name()
                .to_str()
                .unwrap_or("")
                .starts_with("event")
            {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if let Some(device) = device_with_path(path) {
                let (guard, task_guard) = oneshot::channel();
                let transmitter = tx.clone();
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
                                Ok((status, event)) if status == ReadStatus::Success => {
                                    transmitter.send(event).await.unwrap();
                                }
                                Err(_) => println!("Boo"),
                                _ => {}
                            }
                        }
                    }
                });
                task_guards.push(task_guard);
            }
        }

        Self {
            task_receiver,
            _task_guards: task_guards,
        }
    }
}

pub fn device_with_path(path: PathBuf) -> Option<Device> {
    let device = Device::new_from_path(path).ok()?;
    if device.has(evdev_rs::enums::EventType::EV_KEY)
        && device.phys().unwrap_or("").contains("input0")
    {
        return Some(device);
    }
    None
}
