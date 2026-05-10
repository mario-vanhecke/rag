use crate::error::{Error, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct LockOptions {
    /// If true, return Error::LockContention immediately if the lock is held.
    pub no_wait: bool,
    /// How long to wait for the lock if it's held. Ignored when `no_wait`.
    /// Defaults to 60 s when unset.
    pub wait_seconds: Option<u64>,
}

/// Acquire an exclusive file lock at `path`. The returned `File` holds the
/// lock until it's dropped. Caller is responsible for releasing it (drop, or
/// explicit `fs2::FileExt::unlock`).
///
/// Used by `rag index` and `md convert` to serialize long-running write passes
/// against the same vault.
pub fn acquire_lock(path: &Path, opts: &LockOptions) -> Result<File> {
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;

    if opts.no_wait {
        lock_file
            .try_lock_exclusive()
            .map_err(|_| Error::LockContention)?;
        return Ok(lock_file);
    }

    // fs2 doesn't expose a bounded blocking lock; emulate via try_lock with
    // a short polling loop bounded by `wait_seconds`.
    let deadline = Instant::now() + Duration::from_secs(opts.wait_seconds.unwrap_or(60));
    loop {
        match lock_file.try_lock_exclusive() {
            Ok(()) => return Ok(lock_file),
            Err(_) => {
                if Instant::now() >= deadline {
                    return Err(Error::LockContention);
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}
