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

/// IPC Testing
#[cfg(test)]
mod ipc {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    use crate::config::Command;
    use crate::config::command::Reload;
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

        assert_eq!(Reload::new().normalize(), command_pipe.read_command().await.unwrap().normalize());
    }
}
