#![cfg(windows)]

use mem_map::sampler::capture;

#[test]
#[ignore = "slow: reads live OS counters and touches 64 MiB"]
fn native_sampler_tracks_real_private_allocation_and_stable_process_identity() {
    let pid = std::process::id();
    let before = capture(0).expect("Native Windows sample");
    let own = before
        .processes
        .iter()
        .find(|p| p.key.pid == pid)
        .expect("Self is observable");
    assert!(own.key.created > 0);
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
