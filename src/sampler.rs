use crate::model::{ProcessKey, ProcessSample, Snapshot};
use std::io;
use std::mem::{size_of, zeroed};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_NO_MORE_FILES, FILETIME, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX2},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{
            GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
};

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: constructed only from owned, valid handles; never a pseudo-handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub fn capture(at: u32) -> io::Result<Snapshot> {
    // SAFETY: all API buffers are initialized and sized for the documented structs.
    // Every opened handle is RAII-owned, and none of these APIs retain our buffers.
    unsafe {
        let mut own: PROCESS_MEMORY_COUNTERS_EX2 = zeroed();
        own.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
        if K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut own as *mut PROCESS_MEMORY_COUNTERS_EX2).cast(),
            own.cb,
        ) == 0
        {
            return Err(io::Error::other(format!(
                "Private RAM counters unavailable ({}). Requires Windows 10/11 22H2 with September 2023 updates or newer.",
                io::Error::last_os_error()
            )));
        }
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = OwnedHandle(snapshot);
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot.0, &mut entry) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut frame = Snapshot {
            at,
            ..Default::default()
        };
        let mut path = vec![0u16; 32768];
        loop {
            if entry.th32ProcessID != 0 {
                match read_process(entry.th32ProcessID, entry.th32ParentProcessID, &mut path) {
                    Ok(process) => frame.processes.push(process),
                    Err(_) => frame.inaccessible += 1,
                }
            }
            if Process32NextW(snapshot.0, &mut entry) == 0 {
                if GetLastError() != ERROR_NO_MORE_FILES {
                    return Err(io::Error::last_os_error());
                }
                break;
            }
        }
        // Read physical totals last to reduce skew between the pie and process scan.
        let mut memory: MEMORYSTATUSEX = zeroed();
        memory.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut memory) == 0 {
            return Err(io::Error::last_os_error());
        }
        frame.total = memory.ullTotalPhys;
        frame.available = memory.ullAvailPhys;
        Ok(frame)
    }
}

unsafe fn read_process(pid: u32, parent_pid: u32, path: &mut [u16]) -> io::Result<ProcessSample> {
    // SAFETY: caller provides a writable path buffer; handle ownership remains local.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = OwnedHandle(handle);
        let mut creation: FILETIME = zeroed();
        let mut exit = zeroed();
        let mut kernel = zeroed();
        let mut user = zeroed();
        if GetProcessTimes(handle.0, &mut creation, &mut exit, &mut kernel, &mut user) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut counters: PROCESS_MEMORY_COUNTERS_EX2 = zeroed();
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
        if K32GetProcessMemoryInfo(
            handle.0,
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX2).cast(),
            counters.cb,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut length = path.len() as u32;
        if QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut length) == 0 {
            return Err(io::Error::last_os_error());
        }
        let image = String::from_utf16_lossy(&path[..length as usize]);
        let name = image.rsplit('\\').next().unwrap_or(&image).to_string();
        Ok(ProcessSample {
            key: ProcessKey {
                pid,
                created: ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64,
            },
            name,
            image,
            private: counters.PrivateWorkingSetSize as u64,
            commit: counters.PrivateUsage as u64,
            working_set: counters.WorkingSetSize as u64,
            parent_pid,
        })
    }
}
