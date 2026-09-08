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

pub(super) struct ConsoleKeyboard {
    tty: File,
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
        let mut original_mode = 0;
        // SAFETY: KDGKBMODE writes an int to a valid, aligned pointer.
        cvt(unsafe { libc::ioctl(tty.as_raw_fd(), KDGKBMODE, &mut original_mode) })?;
        // SAFETY: KDSKBMODE takes the mode value, not a pointer.
        cvt(unsafe { libc::ioctl(tty.as_raw_fd(), KDSKBMODE, K_OFF) })?;
        let guard = Self { tty, original_mode };
        // Discard input queued before the event loop started as well.
        guard.flush()?;
        Ok(Some(guard))
    }

    fn flush(&self) -> io::Result<()> {
        // SAFETY: tty is an open terminal descriptor and TCIFLUSH is valid.
        cvt(unsafe { libc::tcflush(self.tty.as_raw_fd(), libc::TCIFLUSH) })
    }
}

impl Drop for ConsoleKeyboard {
    fn drop(&mut self) {
        // Never release possibly sensitive queued input to the waiting shell.
        if let Err(e) = self.flush() {
            eprintln!("slint linuxkms: could not flush console input: {e}");
            return;
        }
        // SAFETY: the descriptor is still open and this is the saved kernel mode.
        if let Err(e) =
            cvt(unsafe { libc::ioctl(self.tty.as_raw_fd(), KDSKBMODE, self.original_mode) })
        {
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
