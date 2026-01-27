use std::fs::{self, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;

const WIPE_BUFFER_SIZE: usize = 8192;

pub fn secure_delete_file_blocking(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let mut file = OpenOptions::new().write(true).open(path)?;
    let len = file.metadata()?.len();

    if len > 0 {
        file.seek(SeekFrom::Start(0))?;
        let buffer = vec![0u8; WIPE_BUFFER_SIZE];
        let mut remaining = len;

        while remaining > 0 {
            let chunk = std::cmp::min(remaining, buffer.len() as u64) as usize;
            file.write_all(&buffer[..chunk])?;
            remaining -= chunk as u64;
        }

        file.flush()?;
        file.sync_all()?;
    }

    drop(file);
    fs::remove_file(path)?;
    Ok(())
}

pub async fn secure_delete_file(path: &Path) -> io::Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || secure_delete_file_blocking(&path))
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn secure_delete_removes_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("secret.txt");

        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&path)
                .expect("open file");
            file.write_all(b"secret").expect("write secret");
            file.flush().expect("flush");
        }

        secure_delete_file_blocking(&path).expect("secure delete");
        assert!(!path.exists());
    }

    #[test]
    fn secure_delete_missing_file_is_ok() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("missing.txt");

        secure_delete_file_blocking(&path).expect("secure delete");
        assert!(!path.exists());
    }
}
