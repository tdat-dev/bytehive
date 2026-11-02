#![cfg(windows)]
//! RAII wrapper over a Win32 Job Object configured to KILL the whole process
//! tree when the (single) job handle closes — i.e. when this `Job` drops or the
//! owning process exits. The tree also runs at below-normal priority. `HANDLE` has no `Drop` in the `windows` crate, so we
//! close it ourselves; that close is exactly the kill trigger.

use core::ffi::c_void;
use std::mem::size_of;
use std::os::windows::io::RawHandle;

use windows::core::{Result, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PRIORITY_CLASS,
};
use windows::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;

pub struct Job {
    handle: HANDLE,
}

// The job handle is a kernel handle; moving it across threads is sound.
unsafe impl Send for Job {}

impl Job {
    pub fn new_kill_on_close() -> Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(None, PCWSTR::null())?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            // Field type is JOB_OBJECT_LIMIT (verified); direct assign compiles.
            // Below normal for the whole tree, children an agent starts later
            // included (a Gradle or cargo build): it runs flat out while the
            // PC is idle, and the mouse, keys and the user's own apps always
            // go first, so a heavy build no longer makes the machine stutter.
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PRIORITY_CLASS;
            info.BasicLimitInformation.PriorityClass = BELOW_NORMAL_PRIORITY_CLASS.0;
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )?;
            Ok(Job { handle })
        }
    }

    /// Assign a process (by its raw OS HANDLE) to the job. The handle is only
    /// read transiently by the kernel; ownership stays with the caller.
    pub fn assign_raw(&self, process: RawHandle) -> Result<()> {
        unsafe { AssignProcessToJobObject(self.handle, HANDLE(process)) }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // Closing the last job handle triggers KILL_ON_JOB_CLOSE.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
