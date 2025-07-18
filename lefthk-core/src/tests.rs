/// Test Helpers
#[cfg(test)]
pub(crate) mod test {
    pub fn temp_path() -> std::io::Result<std::path::PathBuf> {
        tempfile::Builder::new()
            .tempfile_in("../target")
            .expect("Blocking task joined")
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
    async fn simulate_command_sending() {
        let pipe_file = temp_path().unwrap();
        let mut command_pipe = Pipe::new(pipe_file.clone()).await.unwrap();
        let mut pipe = fs::OpenOptions::new()
            .write(true)
            .open(&pipe_file)
            .await
            .unwrap();

        let command = Reload::new();

        let normalized = command.normalize();
        pipe.write_all(format!("{normalized}\n").as_bytes())
            .await
            .unwrap();
        pipe.flush().await.unwrap();
        let denormalized = command_pipe.get_next_command().await.unwrap();

        assert_eq!(command.normalize(), denormalized.normalize());
    }
}
