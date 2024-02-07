use evdev_rs::{Device, DeviceWrapper};
use std::cmp::Ordering;
use std::future::Future;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::pin::Pin;
use std::ptr;
use std::sync::Arc;
use std::task::{Context, Waker};
use tokio::sync::{oneshot, Notify};
use tokio::time::Duration;

use crate::errors::{self, Error, LeftError, Result};

pub struct EvDev {
    pub devices: Vec<Device>,
    pub task_notify: Arc<Notify>,
    _task_guards: Vec<oneshot::Receiver<()>>,
}

// impl From<(PathBuf, Device)> for EvDev {
//     fn from(value: (PathBuf, Device)) -> Self {
//         Self {
//             name: value.1.name().unwrap_or("").to_string(),
//             phys: value.1.physical_path().unwrap_or("").to_string(),
//             path: value.0,
//         }
//     }
// }

impl EvDev {
    pub fn new() -> Self {
        let task_notify = Arc::new(Notify::new());

        let mut task_guards: Vec<oneshot::Receiver<()>> = vec![];
        let mut devices = vec![];
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

            match device_with_path(path) {
                Ok(item) => devices.push(item),
                Err(err) => tracing::error!("{:#}", err),
            }
        }
        devices
            .iter()
            .filter(|device| {
                device.has(evdev_rs::enums::EventType::EV_KEY)
                    && device.phys().unwrap().contains("input0")
            })
            .for_each(|device| {
                let (guard, task_guard) = oneshot::channel();
                let notify = task_notify.clone();
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
                tokio::task::spawn_blocking(move || loop {
                    if guard.is_closed() {
                        println!("Bye");
                        return;
                    }

                    if let Err(err) = poll.poll(&mut events, Some(timeout)) {
                        tracing::warn!("Xlib socket poll failed with {:?}", err);
                        continue;
                    }

                    events
                        .iter()
                        .filter(|event| SERVER == event.token())
                        .for_each(|_| notify.notify_one());
                });
                task_guards.push(task_guard);
            });

        Self {
            devices,
            task_notify,
            _task_guards: task_guards,
        }
    }

    pub fn wait_readable(&mut self) -> Pin<Box<dyn Future<Output = ()>>> {
        let task_notify = self.task_notify.clone();
        Box::pin(async move {
            task_notify.notified().await;
        })
    }

    // fn obtain_device_list() -> Result<Vec<EvDev>> {
    //     let mut devices: Vec<EvDev> = evdev::enumerate()
    //         .filter(|(_, device)| {
    //             device
    //                 .supported_keys()
    //                 .map_or(false, |keys| keys.contains(evdev::Key::KEY_ENTER))
    //         })
    //         .map(|device| Self::from(device))
    //         .collect();
    //
    //     // Order by name, but when multiple devices have the same name,
    //     // order by the event device unit number
    //     devices.sort_by(|a, b| {
    //         match event_number_from_path(&a.path).cmp(&event_number_from_path(&b.path)) {
    //             Ordering::Equal => {
    //                 event_number_from_path(&a.path).cmp(&event_number_from_path(&b.path))
    //             }
    //             different => different,
    //         }
    //     });
    //     Ok(devices)
    // }
}

pub fn device_with_path(path: PathBuf) -> Result<Device> {
    let f = std::fs::File::open(&path)?;
    Ok(Device::new_from_path(path)?)
}

// fn event_number_from_path(path: &PathBuf) -> u32 {
//     match path.to_str() {
//         Some(s) => match s.rfind("event") {
//             Some(idx) => s[idx + 5..].parse().unwrap_or(0),
//             None => 0,
//         },
//         None => 0,
//     }
// }
//
// pub fn list_devices() -> Error {
//     let devices = EvDev::obtain_device_list()?;
//     for item in &devices {
//         println!("Name: {}", item.name);
//         println!("Path: {}", item.path.display());
//         println!("Phys: {}", item.phys);
//         println!();
//     }
//     Ok(())
// }
