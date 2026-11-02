//! Bounded reads and atomic replacement of user files.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub fn read_limited(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected a regular file",
        ));
    }
    let file = File::open(path)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File exceeds the size limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File exceeds the size limit",
        ));
    }
    Ok(bytes)
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = parent_directory(path);
    match fs::metadata(path) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Expected a regular file",
            ));
        }
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".rpp-")
        .tempfile_in(parent)?;
    temporary.write_all(bytes)?;
    if let Ok(metadata) = fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace(path, bytes)
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

pub fn write_with_backup(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // Never move the original out of the way: a failed replacement must leave it readable.
    match fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Expected a regular file",
                ));
            }
            let mut original = File::open(path)?;
            let backup = backup_path(path);
            let mut temporary = tempfile::Builder::new()
                .prefix(".rpp-backup-")
                .tempfile_in(parent_directory(path))?;
            io::copy(&mut original, &mut temporary)?;
            temporary
                .as_file()
                .set_permissions(original.metadata()?.permissions())?;
            temporary.as_file().sync_all()?;
            temporary.persist(backup).map_err(|error| error.error)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    replace(path, bytes)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{backup_path, read_limited, write_atomic, write_with_backup};

    #[test]
    fn replacement_keeps_backup_and_does_not_touch_legacy_temporary_names() {
        let directory = tempfile::tempdir().expect("test directory");
        let path = directory.path().join("plan.rpp");
        let unrelated = path.with_extension("rpp.tmp");
        fs::write(&path, b"old plan").expect("old file");
        fs::write(&unrelated, b"unrelated file").expect("unrelated file");
        write_with_backup(&path, b"new plan").expect("save");
        assert_eq!(fs::read(&path).expect("plan"), b"new plan");
        assert_eq!(fs::read(backup_path(&path)).expect("backup"), b"old plan");
        assert_eq!(fs::read(unrelated).expect("unrelated"), b"unrelated file");
    }

    #[test]
    fn failed_backup_keeps_the_original_plan() {
        let directory = tempfile::tempdir().expect("test directory");
        let path = directory.path().join("plan.rpp");
        fs::write(&path, b"original").expect("original");
        fs::create_dir(backup_path(&path)).expect("block backup destination");
        assert!(write_with_backup(&path, b"replacement").is_err());
        assert_eq!(fs::read(path).expect("original remains"), b"original");
    }

    #[test]
    fn unusual_extensions_and_bounded_reads_are_safe() {
        let directory = tempfile::tempdir().expect("test directory");
        let path = directory.path().join("plan.rpp.bak");
        write_atomic(&path, b"original").expect("create");
        write_with_backup(&path, b"replacement").expect("replace");
        assert_eq!(fs::read(backup_path(&path)).expect("backup"), b"original");
        assert!(read_limited(&path, 2).is_err());
        assert_eq!(
            read_limited(&path, 11).expect("bounded read"),
            b"replacement"
        );
    }
    #[cfg(unix)]
    #[test]
    fn special_files_are_rejected_without_waiting_for_a_pipe_writer() {
        let directory = tempfile::tempdir().expect("test directory");
        let pipe = directory.path().join("not-a-project.rpp");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&pipe)
                .status()
                .expect("mkfifo")
                .success()
        );
        assert!(read_limited(&pipe, 1024).is_err());
        assert!(write_with_backup(&pipe, b"data").is_err());
        assert!(write_atomic(&pipe, b"data").is_err());
    }
}
