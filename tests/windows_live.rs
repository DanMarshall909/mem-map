#![cfg(windows)]

use mem_map::sampler::capture;

// Both tests observe process-global handles; an owned child must not skew the
// other test's before/after handle counts when the test harness runs in parallel.
static NATIVE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
#[ignore = "slow: captures a real owned child process and its parent"]
fn native_capture_names_the_actual_parent_of_an_owned_child() {
    let _guard = NATIVE_TEST_LOCK.lock().unwrap();
    use mem_map::model::History;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    struct ChildProbe(std::process::Child);
    impl Drop for ChildProbe {
        fn drop(&mut self) {
            // EOF releases the test-owned command, including during an assertion panic.
            drop(self.0.stdin.take());
            let _ = self.0.wait();
        }
    }
    let command = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32")
        .join("cmd.exe");
    let probe = ChildProbe(
        Command::new(command)
            .args(["/D", "/Q", "/C", "set /p memmap_parent_probe="])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .unwrap(),
    );
    let mut history = History::default();
    history.observe(capture(0).expect("Native parent/child capture"));
    let child = history
        .tracks
        .values()
        .find(|t| t.key.pid == probe.0.id())
        .expect("Owned child is visible");
    assert_eq!(child.parent_pid, std::process::id());
    let executable = std::env::current_exe().unwrap();
    let expected = executable.file_name().unwrap().to_string_lossy();
    assert_eq!(child.parent_name.as_deref(), Some(expected.as_ref()));
    assert_eq!(child.name, "cmd.exe");
}

#[test]
#[ignore = "slow: reads live OS counters and touches 64 MiB"]
fn native_sampler_tracks_real_private_allocation_and_stable_process_identity() {
    let _guard = NATIVE_TEST_LOCK.lock().unwrap();
    let pid = std::process::id();
    let before = capture(0).expect("Native Windows sample");
    assert!(
        before.complete_scan,
        "The live fixture requires a completed OS enumeration"
    );
    assert!(before.system_error.is_none());
    let own = before
        .processes
        .iter()
        .find(|p| p.key.pid == pid)
        .expect("Self is observable");
    assert!(own.key.created > 0);
    assert_eq!(
        own.key.generation, 0,
        "An OS creation time is a known identity"
    );
    let enumerated = before
        .observed
        .iter()
        .find(|p| p.pid == pid)
        .expect("Own process has enumeration metadata");
    assert_eq!(enumerated.key, Some(own.key));
    assert_eq!(enumerated.name, own.name);
    assert_eq!(enumerated.image.as_deref(), Some(own.image.as_str()));
    assert!(before.observed.len() >= before.processes.len());
    assert!(own.image.to_lowercase().contains("windows_live"));
    assert!(own.private > 0 && own.working_set >= own.private);
    assert!(before.total > before.available && before.available > 0);
    let key = own.key;
    let private = own.private;
    let commit = own.commit;
    let mut allocation = vec![0u8; 64 * 1024 * 1024];
    for page in allocation.chunks_mut(4096) {
        page[0] = 123;
    }
    std::hint::black_box(&allocation);
    let after = capture(2).unwrap();
    let own = after.processes.iter().find(|p| p.key == key).unwrap();
    assert!(
        own.private >= private + 60 * 1024 * 1024,
        "Physical allocation must appear as private RAM"
    );
    assert!(
        own.commit >= commit + 60 * 1024 * 1024,
        "Commit must track allocation separately"
    );
    assert!(after.processes.len() > 10);
    // A monitor must release its snapshot and per-process handles every scan.
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
        let mut before_handles = 0;
        assert_ne!(
            GetProcessHandleCount(GetCurrentProcess(), &mut before_handles),
            0
        );
        for at in 3..8 {
            std::hint::black_box(capture(at).unwrap());
        }
        let mut after_handles = 0;
        assert_ne!(
            GetProcessHandleCount(GetCurrentProcess(), &mut after_handles),
            0
        );
        assert!(
            after_handles <= before_handles + 2,
            "Repeated scans must not retain process handles"
        );
    }
}
