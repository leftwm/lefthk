use crate::{
    config::Command,
    errors::{LeftError, Result},
};
use std::path::{Path, PathBuf};
use tokio::{
    fs,
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
};

pub struct Pipe {
    pipe_file: PathBuf,
    rx: mpsc::UnboundedReceiver<Command>,
}

impl Drop for Pipe {
    fn drop(&mut self) {
        use std::os::unix::fs::OpenOptionsExt;
        self.rx.close();

        // Open fifo for write to unblock pending open for read operation that prevents tokio runtime
        // from shutting down.
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(nix::fcntl::OFlag::O_NONBLOCK.bits())
            .open(self.pipe_file.clone());
    }
}

impl Pipe {
    /// Create and listen to the named pipe.
    /// # Errors
    ///
    /// Will error if unable to `mkfifo`, likely a filesystem issue
    /// such as inadequate permissions.
    pub async fn new() -> Result<Self> {
        let pipe_file =
            xdg::BaseDirectories::with_prefix("lefthk")?.place_runtime_file("commands.pipe")?;
        let _ = fs::remove_file(pipe_file.as_path()).await;
        nix::unistd::mkfifo(&pipe_file, nix::sys::stat::Mode::S_IRWXU)?;

        let path = pipe_file.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while !tx.is_closed() {
                read_from_pipe(&path, &tx).await;
            }
            fs::remove_file(path).await.ok();
        });

        Ok(Self { pipe_file, rx })
    }

    pub async fn read_command(&mut self) -> Option<Command> {
        self.rx.recv().await
    }
}

async fn read_from_pipe(pipe_file: &Path, tx: &mpsc::UnboundedSender<Command>) -> Option<()> {
    let file = fs::File::open(pipe_file).await.ok()?;
    let mut lines = BufReader::new(file).lines();

    while let Some(line) = lines.next_line().await.ok()? {
        let cmd = match parse_command(&line) {
            Ok(cmd) => cmd,
            Err(err) => {
                log::error!("An error occurred while parsing the command: {}", err);
                return None;
            }
        };
        tx.send(cmd).ok()?;
    }

    Some(())
}

fn parse_command(string: &str) -> Result<Command> {
    match string {
        "Reload" => Ok(Command::Reload),
        "Kill" => Ok(Command::Kill),
        _ => Err(LeftError::CommandNotFound),
    }
}
