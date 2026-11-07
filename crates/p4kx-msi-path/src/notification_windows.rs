// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#![deny(unsafe_op_in_unsafe_fn)]
#![expect(
    unsafe_code,
    reason = "ADR-0354: the fixed bounded environment notification is approved"
)]

use std::time::Instant;

use windows_sys::Win32::Foundation::{self, HWND, LPARAM};
use windows_sys::Win32::System::Threading;
use windows_sys::Win32::UI::WindowsAndMessaging;
use windows_sys::core::BOOL;

use crate::notification;
use crate::notification::{Delivery, Enumeration, FailureKind, Stop, Transport};

struct SystemClock;

impl notification::Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

struct Windows;

/// Notifies top-level desktop windows using one shared wait budget.
pub(crate) fn environment_changed() -> Result<notification::Report, notification::Failure> {
    notification::notify(&mut Windows, &SystemClock)
}

struct Collection {
    deadline: Instant,
    maximum: usize,
    windows: Vec<HWND>,
    stop: Stop,
}

impl Transport for Windows {
    type Window = HWND;

    fn enumerate(
        &mut self,
        deadline: Instant,
        maximum: usize,
    ) -> Result<Enumeration<HWND>, FailureKind> {
        let mut collection = Collection {
            deadline,
            maximum,
            windows: Vec::with_capacity(maximum),
            stop: Stop::Complete,
        };
        let parameter = (&raw mut collection) as LPARAM;
        // SAFETY: EnumWindows invokes collect synchronously with this live exclusive pointer.
        let result = unsafe {
            Foundation::SetLastError(0);
            WindowsAndMessaging::EnumWindows(Some(collect), parameter)
        };
        if result == 0 && collection.stop == Stop::Complete {
            // SAFETY: GetLastError reads this thread's immediately preceding native result.
            return Err(FailureKind::Native(unsafe { Foundation::GetLastError() }));
        }
        Ok(Enumeration {
            windows: collection.windows,
            stop: collection.stop,
        })
    }

    fn send(&mut self, window: HWND, timeout_ms: u32) -> Result<Delivery, FailureKind> {
        // SAFETY: The handles originate from EnumWindows; no native output pointer is required.
        let (thread, error) = unsafe {
            Foundation::SetLastError(0);
            let thread =
                WindowsAndMessaging::GetWindowThreadProcessId(window, std::ptr::null_mut());
            (thread, Foundation::GetLastError())
        };
        let Some(thread) =
            notification::thread_lookup(thread, error, Foundation::ERROR_INVALID_WINDOW_HANDLE)?
        else {
            return Ok(Delivery::WindowGone);
        };
        // SAFETY: GetCurrentThreadId has no pointer or handle preconditions.
        if thread == unsafe { Threading::GetCurrentThreadId() } {
            return Err(FailureKind::CurrentThread);
        }
        // SAFETY: Environment is static NUL-terminated UTF16; the system marshals WM_SETTINGCHANGE.
        let result = unsafe {
            Foundation::SetLastError(0);
            WindowsAndMessaging::SendMessageTimeoutW(
                window,
                WindowsAndMessaging::WM_SETTINGCHANGE,
                0,
                notification::ENVIRONMENT.as_ptr() as LPARAM,
                WindowsAndMessaging::SMTO_ABORTIFHUNG
                    | WindowsAndMessaging::SMTO_BLOCK
                    | WindowsAndMessaging::SMTO_ERRORONEXIT,
                timeout_ms,
                std::ptr::null_mut(),
            )
        };
        if result != 0 {
            return Ok(Delivery::Acknowledged);
        }
        // SAFETY: GetLastError reads this thread's immediately preceding native result.
        match unsafe { Foundation::GetLastError() } {
            Foundation::ERROR_TIMEOUT => Ok(Delivery::TimedOut),
            Foundation::ERROR_INVALID_WINDOW_HANDLE => Ok(Delivery::WindowGone),
            code => Err(FailureKind::Native(code)),
        }
    }
}

unsafe extern "system" fn collect(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: enumerate passes its live exclusive Collection for this synchronous callback only.
    let collection = unsafe { &mut *(parameter as *mut Collection) };
    if Instant::now() >= collection.deadline {
        collection.stop = Stop::Deadline;
        return 0;
    }
    if collection.windows.len() >= collection.maximum {
        collection.stop = Stop::WindowLimit;
        return 0;
    }
    collection.windows.push(window);
    1
}
