//! RT-safe wake signal: the audio (or MIDI) thread calls `notify()` — one
//! non-blocking `write` on an eventfd, no allocation, no lock — and a waiter
//! thread blocks in `wait()` until it fires, then pokes the UI event loop.

use std::os::raw::c_void;
use std::sync::Arc;

#[cfg(target_os = "linux")]
mod sys {
    use super::c_void;
    unsafe extern "C" {
        pub fn eventfd(initval: u32, flags: i32) -> i32;
        pub fn write(fd: i32, buf: *const c_void, n: usize) -> isize;
        pub fn read(fd: i32, buf: *mut c_void, n: usize) -> isize;
        pub fn close(fd: i32) -> i32;
    }
    pub const EFD_CLOEXEC: i32 = 0o2000000;
}

struct Fd(i32);

impl Drop for Fd {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        // SAFETY: fd is one we created and nobody else closes.
        unsafe {
            sys::close(self.0);
        }
    }
}

/// The signalling half. Clone freely; `notify` is safe on any thread.
#[derive(Clone)]
pub struct WakeNotifier {
    fd: Arc<Fd>,
}

/// The blocking half.
pub struct WakeWaiter {
    fd: Arc<Fd>,
}

/// Create a notifier/waiter pair. Returns `None` if the OS refuses an eventfd.
pub fn wake_pair() -> Option<(WakeNotifier, WakeWaiter)> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: plain syscall with constant flags.
        let fd = unsafe { sys::eventfd(0, sys::EFD_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        let fd = Arc::new(Fd(fd));
        Some((WakeNotifier { fd: fd.clone() }, WakeWaiter { fd }))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

impl WakeNotifier {
    /// Wake the waiter. Never blocks, never allocates.
    #[inline]
    pub fn notify(&self) {
        #[cfg(target_os = "linux")]
        {
            let one: u64 = 1;
            // SAFETY: writing 8 bytes from a local to an eventfd; the counter
            // cannot overflow in practice, so this does not block.
            unsafe {
                sys::write(self.fd.0, &one as *const u64 as *const c_void, 8);
            }
        }
    }
}

impl WakeWaiter {
    /// Block until notified at least once since the last `wait`.
    pub fn wait(&self) {
        #[cfg(target_os = "linux")]
        {
            let mut v: u64 = 0;
            // SAFETY: reading 8 bytes into a local from our eventfd.
            unsafe {
                sys::read(self.fd.0, &mut v as *mut u64 as *mut c_void, 8);
            }
        }
        #[cfg(not(target_os = "linux"))]
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_wakes_waiter_across_threads() {
        let (n, w) = wake_pair().expect("eventfd");
        let t = std::thread::spawn(move || {
            w.wait();
            w.wait();
            true
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        n.notify();
        std::thread::sleep(std::time::Duration::from_millis(20));
        n.notify();
        assert!(t.join().unwrap());
    }
}
