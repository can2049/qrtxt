//! Win32 console event source (FR-3.12, FR-3.13).
//!
//! Replies arrive as console input records, so `ReadConsoleInputW` is used
//! instead of `ReadFile`: focus, mouse, and buffer-resize records are visible
//! and ignored rather than polluting the reply or blocking the read.
//!
//! This is the crate's only unsafe code. The three functions that make Win32
//! calls opt in with `allow(unsafe_code)`; the crate root denies unsafe
//! everywhere else.

use std::io;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Console::{
    GetNumberOfConsoleInputEvents, INPUT_RECORD, KEY_EVENT, ReadConsoleInputW,
};

use crate::event::{Event, EventSource};

/// How long to wait between polls of an empty console input queue.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Console input records fetched per `ReadConsoleInputW` call.
const BATCH: usize = 32;

/// The controlling console input, opened for the lifetime of a probe.
pub(crate) struct ConsoleEvents {
    input: ConsoleHandle,
}

impl ConsoleEvents {
    pub(crate) fn open() -> io::Result<Self> {
        Ok(Self {
            input: ConsoleHandle::open("CONIN$")?,
        })
    }
}

impl EventSource for ConsoleEvents {
    #[allow(unsafe_code)]
    fn poll(&mut self) -> io::Result<Vec<Event>> {
        let mut available = 0u32;
        // SAFETY: `self.input` owns a live console input handle.
        if unsafe { GetNumberOfConsoleInputEvents(self.input.0, &mut available) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if available == 0 {
            std::thread::sleep(POLL_INTERVAL);
            return Ok(Vec::new());
        }

        let mut records = [INPUT_RECORD::default(); BATCH];
        let mut read = 0u32;
        // SAFETY: `records` has room for `BATCH` records and `read` receives the
        // number of records written into it.
        if unsafe { ReadConsoleInputW(self.input.0, records.as_mut_ptr(), BATCH as u32, &mut read) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }

        let mut events = Vec::with_capacity(read as usize);
        for record in records.iter().take(read as usize) {
            let is_key = u32::from(record.EventType) == KEY_EVENT;
            let (down, character, repeat) = if is_key {
                // SAFETY: `EventType` selects the `KeyEvent` member of the union.
                let key = unsafe { record.Event.KeyEvent };
                // SAFETY: a key event carries its character in this union member.
                (
                    key.bKeyDown != 0,
                    unsafe { key.uChar.UnicodeChar },
                    key.wRepeatCount,
                )
            } else {
                (false, 0, 0)
            };
            events.push(Event::from_key_record(is_key, down, character, repeat));
        }
        Ok(events)
    }
}

/// An owned console input handle, closed on drop.
struct ConsoleHandle(HANDLE);

impl ConsoleHandle {
    #[allow(unsafe_code)]
    fn open(name: &str) -> io::Result<Self> {
        let mut wide: Vec<u16> = name.encode_utf16().collect();
        wide.push(0);
        // SAFETY: `wide` is NUL-terminated and outlives the call; the security
        // and template arguments are null; the returned handle is checked below.
        let raw = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(raw))
    }
}

#[allow(unsafe_code)]
impl Drop for ConsoleHandle {
    fn drop(&mut self) {
        // SAFETY: this guard owns a valid console handle, closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}
