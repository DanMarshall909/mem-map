use crate::model::{ProcessKey, ProcessObservation, ProcessSample, Snapshot};
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
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
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

/// Complete a collection cycle even when one independent source is unavailable.
/// A complete enumeration is the only observation that can confirm an exit.
pub fn capture(at: u32) -> io::Result<Snapshot> {
    // SAFETY: sized writable API buffers and exclusively owned RAII handles.
    unsafe {
        let captured_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let mut frame = Snapshot {
            at,
            captured_unix_ms,
            ..Default::default()
        };
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            frame.complete_scan = false;
            frame.collection_error = Some(format!(
                "Process enumeration: {}",
                io::Error::last_os_error()
            ));
        } else {
            let snapshot = OwnedHandle(snapshot);
            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut path = vec![0u16; 32768];
            let mut more = Process32FirstW(snapshot.0, &mut entry) != 0;
            if !more && GetLastError() != ERROR_NO_MORE_FILES {
                frame.complete_scan = false;
                frame.collection_error = Some(format!(
                    "Process enumeration: {}",
                    io::Error::last_os_error()
                ));
            }
            while more {
                if entry.th32ProcessID != 0 {
                    let length = entry
                        .szExeFile
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
                    let (observation, process) = read_process(
                        entry.th32ProcessID,
                        entry.th32ParentProcessID,
                        &name,
                        &mut path,
                    );
                    if let Some(process) = process {
                        frame.processes.push(process);
                    } else {
                        frame.inaccessible += 1;
                    }
                    frame.observed.push(observation);
                }
                more = Process32NextW(snapshot.0, &mut entry) != 0;
                if !more && GetLastError() != ERROR_NO_MORE_FILES {
                    frame.complete_scan = false;
                    frame.collection_error = Some(format!(
                        "Process enumeration interrupted: {}",
                        io::Error::last_os_error()
                    ));
                }
            }
        }
        // Independent physical reading survives a failed process scan; conversely a
        // physical failure must not discard valid instance identities or readings.
        let mut memory: MEMORYSTATUSEX = zeroed();
        memory.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut memory) == 0 {
            frame.system_error = Some(format!("System memory: {}", io::Error::last_os_error()));
        } else {
            frame.total = memory.ullTotalPhys;
            frame.available = memory.ullAvailPhys;
        }
        if let Some(system_error) = &frame.system_error
            && !frame.complete_scan
            && frame.observed.is_empty()
        {
            return Err(io::Error::other(format!(
                "{}; {}",
                system_error,
                frame.collection_error.as_deref().unwrap_or_default()
            )));
        }
        Ok(frame)
    }
}

unsafe fn read_process(
    pid: u32,
    parent_pid: u32,
    enumerated_name: &str,
    path: &mut [u16],
) -> (ProcessObservation, Option<ProcessSample>) {
    let mut observation = ProcessObservation {
        pid,
        key: None,
        name: enumerated_name.into(),
        image: None,
        parent_pid,
        error: None,
    };
    // SAFETY: writable sized API buffers; handle ownership does not escape.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            observation.error = Some(format!("OpenProcess: {}", io::Error::last_os_error()));
            return (observation, None);
        }
        let handle = OwnedHandle(handle);
        let mut creation: FILETIME = zeroed();
        let mut exit = zeroed();
        let mut kernel = zeroed();
        let mut user = zeroed();
        if GetProcessTimes(handle.0, &mut creation, &mut exit, &mut kernel, &mut user) == 0 {
            observation.error = Some(format!("Process identity: {}", io::Error::last_os_error()));
            return (observation, None);
        }
        let key = ProcessKey {
            pid,
            created: ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64,
            generation: 0,
        };
        observation.key = Some(key);
        let mut length = path.len() as u32;
        if QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut length) != 0 {
            let image = String::from_utf16_lossy(&path[..length as usize]);
            observation.name = image.rsplit('\\').next().unwrap_or(&image).into();
            observation.image = Some(image);
        } else {
            observation.error = Some(format!("Executable path: {}", io::Error::last_os_error()));
        }
        let mut counters: PROCESS_MEMORY_COUNTERS_EX2 = zeroed();
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
        if K32GetProcessMemoryInfo(
            handle.0,
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX2).cast(),
            counters.cb,
        ) == 0
        {
            observation.error = Some(format!(
                "Private working set counters: {}. Requires Windows 10/11 22H2 with September 2023 updates or newer.",
                io::Error::last_os_error()
            ));
            return (observation, None);
        }
        let sample = ProcessSample {
            key,
            name: observation.name.clone(),
            image: observation.image.clone().unwrap_or_default(),
            private: counters.PrivateWorkingSetSize as u64,
            commit: counters.PrivateUsage as u64,
            working_set: counters.WorkingSetSize as u64,
            parent_pid,
        };
        (observation, Some(sample))
    }
}
