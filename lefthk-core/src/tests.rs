/// Test Helpers
#[cfg(test)]
pub(crate) mod test {
    pub async fn temp_path() -> std::io::Result<std::path::PathBuf> {
        tokio::task::spawn_blocking(|| tempfile::Builder::new().tempfile_in("../target"))
            .await
            .expect("Blocking task joined")?
            .into_temp_path()
            .keep()
            .map_err(Into::into)
    }
}

/// Config Testing
#[cfg(test)]
mod config {
    use super::test::temp_path;
    use crate::config::Watcher;
    use tokio::{fs, io::AsyncWriteExt};

    #[tokio::test]
    async fn check_watcher() {
        let config_file = temp_path().await.unwrap();
        let watcher = Watcher::new(&config_file).unwrap();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&config_file)
            .await
            .unwrap();
        file.write_all(b"\n").await.unwrap();
        file.flush().await.unwrap();

        assert!(watcher.has_events());
    }
}

/// IPC Testing
#[cfg(test)]
mod ipc {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    use crate::config::Command;
    use crate::ipc::Pipe;

    use super::test::temp_path;

    #[tokio::test]
    async fn read_command() {
        let pipe_file = temp_path().await.unwrap();
        let mut command_pipe = Pipe::new(pipe_file.clone()).await.unwrap();

        let mut pipe = fs::OpenOptions::new()
            .write(true)
            .open(&pipe_file)
            .await
            .unwrap();
        pipe.write_all(b"Reload\n").await.unwrap();
        pipe.flush().await.unwrap();

        assert_eq!(Command::Reload, command_pipe.read_command().await.unwrap());
    }
}
