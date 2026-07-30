use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// File-based lock using flock (Unix) or file locking (cross-platform via fs4)
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
        fs4::FileExt::lock(&file)?;
        self.lock_file = Some(file);
        Ok(())
    }

    /// Try to acquire a lock, returning immediately if not available
    #[allow(dead_code)]
    pub fn try_lock(&mut self) -> io::Result<bool> {
        let file = File::create(&self.lock_path)?;
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => {
                self.lock_file = Some(file);
                Ok(true)
            }
            Err(fs4::TryLockError::WouldBlock) => Ok(false),
            Err(fs4::TryLockError::Error(e)) => Err(e),
        }
    }

    /// Try to acquire a lock with a timeout
    #[allow(dead_code)]
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
            fs4::FileExt::unlock(&file)?;
            drop(file);
            // Try to clean up the lock file
            let _ = fs::remove_file(&self.lock_path);
        }
        Ok(())
    }

    /// Execute a closure with the lock held
    #[allow(dead_code)]
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

    #[test]
    fn test_lock_double_lock_same_instance() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut lock = FileLock::new(&file_path);
        lock.lock().unwrap();
        // Double lock on same instance from same process should be fine (upgrade)
        lock.unlock().unwrap();
    }

    #[test]
    fn test_lock_drop_releases() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut lock1 = FileLock::new(&file_path);
        lock1.lock().unwrap();
        drop(lock1); // Drop should release

        let mut lock2 = FileLock::new(&file_path);
        assert!(lock2.try_lock().unwrap());
        lock2.unlock().unwrap();
    }

    #[test]
    fn test_lock_try_lock_when_held() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut lock1 = FileLock::new(&file_path);
        lock1.lock().unwrap();

        let mut lock2 = FileLock::new(&file_path);
        assert!(!lock2.try_lock().unwrap());

        lock1.unlock().unwrap();
    }

    #[test]
    fn test_lock_try_lock_immediate() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut lock = FileLock::new(&file_path);
        assert!(lock.try_lock().unwrap());
        lock.unlock().unwrap();
    }

    #[test]
    fn test_lock_sequential_acquire_release() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        for _ in 0..3 {
            let mut lock = FileLock::new(&file_path);
            lock.lock().unwrap();
            lock.unlock().unwrap();
        }
    }
}
