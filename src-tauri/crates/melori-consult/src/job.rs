//! Kill the engine together with the app, however the app ends.
//!
//! `RunEvent::Exit` only runs on a clean quit; a crash or a hard kill left the engine
//! (and, on Windows, the real interpreter behind the venv launcher) serving on its port.
//! Every engine process is assigned to one Windows job object with
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; the app holds the only handle, so when the app
//! process goes away the OS closes it and terminates the whole tree. Children of an
//! assigned process join the job automatically.

use std::process::Child;

#[cfg(windows)]
pub struct KillOnCloseJob {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

// The handle is a kernel object reference, usable from any thread.
#[cfg(windows)]
unsafe impl Send for KillOnCloseJob {}
#[cfg(windows)]
unsafe impl Sync for KillOnCloseJob {}

#[cfg(windows)]
impl KillOnCloseJob {
    pub fn new() -> std::io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let job = Self { handle };
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(job)
        }
    }

    /// Put a freshly spawned process (and so its future children) into the job.
    pub fn assign(&self, child: &Child) -> std::io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        let ok = unsafe { AssignProcessToJobObject(self.handle, child.as_raw_handle() as _) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for KillOnCloseJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

/// Elsewhere the engine is a direct child without a launcher; nothing to group.
#[cfg(not(windows))]
pub struct KillOnCloseJob;

#[cfg(not(windows))]
impl KillOnCloseJob {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self)
    }
    pub fn assign(&self, _child: &Child) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::KillOnCloseJob;
    use std::net::{TcpListener, TcpStream};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    #[test]
    fn closing_the_job_kills_the_engine_behind_a_launcher() {
        let port = free_port();
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fake_engine.py");
        // the launcher starts the "engine" as its own child, like a venv python.exe
        let launcher =
            "import subprocess, sys; sys.exit(subprocess.call([sys.executable, sys.argv[1]]))";
        let mut child = Command::new("python")
            .args(["-c", launcher, &script.to_string_lossy()])
            .env("MELORI_ENGINE_TOKEN", "t".repeat(32))
            .env("MELORI_ENGINE_PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let job = KillOnCloseJob::new().unwrap();
        job.assign(&child).unwrap();
        let addr = format!("127.0.0.1:{port}");
        assert!(
            wait_until(Duration::from_secs(15), || TcpStream::connect(&addr)
                .is_ok()),
            "fake engine never came up"
        );
        drop(job); // what the OS does when the app process dies
        let gone = wait_until(Duration::from_secs(5), || {
            TcpStream::connect(&addr).is_err()
        });
        let _ = child.kill();
        let _ = child.wait();
        assert!(gone, "engine behind the launcher survived closing the job");
    }
}
