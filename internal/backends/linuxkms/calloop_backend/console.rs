// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Isolate direct libinput input from the Linux virtual console's line discipline.
//!
//! Reading evdev does not consume the corresponding console keystrokes. Without
//! disabling console translation, passwords can be echoed and queued as commands
//! for the shell after the GUI exits. libseat manages this for seat-backed builds.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;

use nix::libc;

const VT_GETSTATE: libc::c_ulong = 0x5603;
const KDGKBMODE: libc::c_ulong = 0x4b44;
const KDSKBMODE: libc::c_ulong = 0x4b45;
const K_OFF: libc::c_int = 4;

#[repr(C)]
#[derive(Default)]
struct VtState {
    active: libc::c_ushort,
    signal: libc::c_ushort,
    state: libc::c_ushort,
}

pub(super) struct ConsoleKeyboard<T: ConsoleIo = File> {
    tty: T,
    original_mode: libc::c_int,
}

impl ConsoleKeyboard {
    pub(super) fn acquire() -> io::Result<Option<Self>> {
        let control = match open_tty("/dev/tty0") {
            Ok(tty) => tty,
            // Embedded systems without a VT have no console input to isolate.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut state = VtState::default();
        // SAFETY: VT_GETSTATE writes a vt_stat with this exact C layout.
        cvt(unsafe { libc::ioctl(control.as_raw_fd(), VT_GETSTATE, &mut state) })?;
        let tty = open_tty(&format!("/dev/tty{}", state.active))?;
        Self::acquire_tty(tty).map(Some)
    }
}

// Separate the terminal operations so failure paths can be tested without a VT.
pub(super) trait ConsoleIo {
    fn keyboard_mode(&self) -> io::Result<libc::c_int>;
    fn set_keyboard_mode(&self, mode: libc::c_int) -> io::Result<()>;
    fn flush_input(&self) -> io::Result<()>;
}

impl ConsoleIo for File {
    fn keyboard_mode(&self) -> io::Result<libc::c_int> {
        let mut mode = 0;
        // SAFETY: KDGKBMODE writes an int to a valid, aligned pointer.
        cvt(unsafe { libc::ioctl(self.as_raw_fd(), KDGKBMODE, &mut mode) })?;
        Ok(mode)
    }

    fn set_keyboard_mode(&self, mode: libc::c_int) -> io::Result<()> {
        // SAFETY: KDSKBMODE takes the mode value, not a pointer.
        cvt(unsafe { libc::ioctl(self.as_raw_fd(), KDSKBMODE, mode) })
    }

    fn flush_input(&self) -> io::Result<()> {
        // SAFETY: self is an open terminal descriptor and TCIFLUSH is valid.
        cvt(unsafe { libc::tcflush(self.as_raw_fd(), libc::TCIFLUSH) })
    }
}

impl<T: ConsoleIo> ConsoleKeyboard<T> {
    fn acquire_tty(tty: T) -> io::Result<Self> {
        let original_mode = tty.keyboard_mode()?;
        tty.set_keyboard_mode(K_OFF)?;
        let guard = Self { tty, original_mode };
        // Discard input queued before the event loop started as well.
        guard.tty.flush_input()?;
        Ok(guard)
    }
}

impl<T: ConsoleIo> Drop for ConsoleKeyboard<T> {
    fn drop(&mut self) {
        // Keep translation disabled if queued input cannot be discarded.
        if let Err(e) = self.tty.flush_input() {
            eprintln!("slint linuxkms: could not flush console input: {e}");
            return;
        }
        if let Err(e) = self.tty.set_keyboard_mode(self.original_mode) {
            eprintln!("slint linuxkms: could not restore console keyboard mode: {e}");
        }
    }
}

fn open_tty(path: &str) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_CLOEXEC)
        .open(path)
}

fn cvt(result: libc::c_int) -> io::Result<()> {
    if result < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[derive(Default)]
    struct State {
        calls: RefCell<Vec<String>>,
        fail_disable: Cell<bool>,
        failed_flushes_remaining: Cell<u32>,
    }

    impl ConsoleIo for Rc<State> {
        fn keyboard_mode(&self) -> io::Result<libc::c_int> {
            self.calls.borrow_mut().push("get".into());
            Ok(3)
        }
        fn set_keyboard_mode(&self, mode: libc::c_int) -> io::Result<()> {
            self.calls.borrow_mut().push(format!("set {mode}"));
            if mode == K_OFF && self.fail_disable.get() {
                return Err(io::Error::other("disable failed"));
            }
            Ok(())
        }
        fn flush_input(&self) -> io::Result<()> {
            self.calls.borrow_mut().push("flush".into());
            if self.failed_flushes_remaining.get() > 0 {
                self.failed_flushes_remaining.set(self.failed_flushes_remaining.get() - 1);
                return Err(io::Error::other("flush failed"));
            }
            Ok(())
        }
    }

    #[test]
    fn normal_return_and_unwind_flush_before_restoring_the_original_mode() {
        for unwind in [false, true] {
            let state = Rc::new(State::default());
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = ConsoleKeyboard::acquire_tty(state.clone()).unwrap();
                if unwind {
                    panic!("event loop failed");
                }
            }));
            assert_eq!(result.is_err(), unwind);
            assert_eq!(*state.calls.borrow(), ["get", "set 4", "flush", "flush", "set 3"]);
        }
    }

    #[test]
    fn failed_acquisition_does_not_restore_a_mode_it_did_not_change() {
        let state = Rc::new(State::default());
        state.fail_disable.set(true);
        assert!(ConsoleKeyboard::acquire_tty(state.clone()).is_err());
        assert_eq!(*state.calls.borrow(), ["get", "set 4"]);
    }

    #[test]
    fn startup_flush_error_retries_cleanup_before_returning() {
        let state = Rc::new(State::default());
        state.failed_flushes_remaining.set(1);
        assert!(ConsoleKeyboard::acquire_tty(state.clone()).is_err());
        assert_eq!(*state.calls.borrow(), ["get", "set 4", "flush", "flush", "set 3"]);
    }

    #[test]
    fn failed_final_flush_never_releases_queued_input_to_the_shell() {
        let state = Rc::new(State::default());
        let guard = ConsoleKeyboard::acquire_tty(state.clone()).unwrap();
        state.failed_flushes_remaining.set(1);
        drop(guard);
        assert_eq!(*state.calls.borrow(), ["get", "set 4", "flush", "flush"]);
    }
}
