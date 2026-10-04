//! Keeps the runtime's `eprintln!` lifecycle lines off the TUI.
//!
//! runtime-core writes diagnostics to stderr. Console owns the terminal in raw
//! mode on the alternate screen, so those bytes would land on the same tty at
//! the cursor and stay on the chrome until ratatui repaints the cells. While the
//! guard is alive, fd 2 / the STD_ERROR handle points at a log file under the
//! temp dir; dropping the guard puts the original stream back.

use std::fs::File;
use std::path::PathBuf;

pub struct StderrGuard {
    path: PathBuf,
    file: File,
    saved: imp::Saved,
}

pub fn log_path() -> PathBuf {
    std::env::temp_dir().join("commandui-console-stderr.log")
}

impl StderrGuard {
    /// Redirect stderr to the log file. Returns None (stderr untouched) when the
    /// file cannot be opened or the redirect fails.
    pub fn redirect() -> Option<StderrGuard> {
        let path = log_path();
        let file = File::create(&path).ok()?;
        let saved = imp::redirect(&file)?;
        Some(StderrGuard { path, file, saved })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for StderrGuard {
    fn drop(&mut self) {
        imp::restore(&self.saved);
        let _ = &self.file;
    }
}

#[cfg(unix)]
mod imp {
    use std::fs::File;
    use std::os::fd::AsRawFd;

    extern "C" {
        fn dup(fd: i32) -> i32;
        fn dup2(old: i32, new: i32) -> i32;
        fn close(fd: i32) -> i32;
    }

    pub struct Saved(i32);

    pub fn redirect(file: &File) -> Option<Saved> {
        unsafe {
            let saved = dup(2);
            if saved < 0 {
                return None;
            }
            if dup2(file.as_raw_fd(), 2) < 0 {
                close(saved);
                return None;
            }
            Some(Saved(saved))
        }
    }

    pub fn restore(saved: &Saved) {
        unsafe {
            dup2(saved.0, 2);
            close(saved.0);
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle;

    type Handle = isize;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    extern "system" {
        fn GetStdHandle(which: u32) -> Handle;
        fn SetStdHandle(which: u32, handle: Handle) -> i32;
    }

    pub struct Saved(Handle);

    pub fn redirect(file: &File) -> Option<Saved> {
        unsafe {
            let saved = GetStdHandle(STD_ERROR_HANDLE);
            if SetStdHandle(STD_ERROR_HANDLE, file.as_raw_handle() as Handle) == 0 {
                return None;
            }
            Some(Saved(saved))
        }
    }

    pub fn restore(saved: &Saved) {
        unsafe {
            SetStdHandle(STD_ERROR_HANDLE, saved.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_is_under_the_temp_dir() {
        assert!(log_path().starts_with(std::env::temp_dir()));
    }
}
