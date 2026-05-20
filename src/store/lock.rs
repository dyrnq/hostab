use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// File-based lock using flock (Unix) or file locking (cross-platform via fs2)
pub struct FileLock {
    lock_path: PathBuf,
    lock_file: Option<File>,
}

impl FileLock {
    /// Create a new FileLock for the given file path
    pub fn new(file_path: &Path) -> Self {
        let lock_path = file_path.with_extension("lock");
        Self {
            lock_path,
            lock_file: None,
        }
    }

    /// Acquire an exclusive lock, blocking until available
    pub fn lock(&mut self) -> io::Result<()> {
        let file = File::create(&self.lock_path)?;
        fs2::FileExt::lock_exclusive(&file)?;
        self.lock_file = Some(file);
        Ok(())
    }

    /// Try to acquire a lock, returning immediately if not available
    pub fn try_lock(&mut self) -> io::Result<bool> {
        let file = File::create(&self.lock_path)?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => {
                self.lock_file = Some(file);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Try to acquire a lock with a timeout
    pub fn lock_with_timeout(&mut self, timeout: Duration) -> io::Result<bool> {
        let start = std::time::Instant::now();
        loop {
            match self.try_lock() {
                Ok(true) => return Ok(true),
                Ok(false) => {
                    if start.elapsed() >= timeout {
                        return Ok(false);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Release the lock
    pub fn unlock(&mut self) -> io::Result<()> {
        if let Some(file) = self.lock_file.take() {
            fs2::FileExt::unlock(&file)?;
            drop(file);
            // Try to clean up the lock file
            let _ = fs::remove_file(&self.lock_path);
        }
        Ok(())
    }

    /// Execute a closure with the lock held
    pub fn with_lock<F, T>(&mut self, f: F) -> io::Result<T>
    where
        F: FnOnce() -> io::Result<T>,
    {
        self.lock()?;
        let result = f();
        self.unlock()?;
        result
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_lock_acquire_release() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut lock = FileLock::new(&file_path);

        assert!(lock.try_lock().unwrap());
        lock.unlock().unwrap();
    }

    #[test]
    fn test_lock_timeout() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut lock1 = FileLock::new(&file_path);

        lock1.lock().unwrap();

        // Try from a different lock instance (same lock file)
        let mut lock2 = FileLock::new(&file_path);
        let result = lock2.lock_with_timeout(Duration::from_millis(100));
        assert!(!result.unwrap());

        lock1.unlock().unwrap();
    }
}
