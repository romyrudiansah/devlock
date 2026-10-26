//! Menjalankan service sebagai proses latar dalam Windows Job Object, sehingga seluruh
//! pohon proses (mis. master + worker nginx) bisa dihentikan bersama dan otomatis ikut
//! mati bila daemon DevLock berhenti.

use std::fs::OpenOptions;
use std::process::{Child, Command as StdCommand, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::services::Command;

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const DETACHED_PROCESS: u32 = 0x0000_0008;
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
pub const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

pub struct Process {
    pub child: Child,
    job: Job,
}

impl Process {
    pub fn spawn(cmd: &Command, log: &std::path::Path) -> Result<Self> {
        if let Some(dir) = log.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .with_context(|| format!("gagal membuka log {}", log.display()))?;
        let err = out.try_clone()?;

        let mut command = std_command(cmd);
        command.stdin(Stdio::null()).stdout(out).stderr(err);
        let child = command
            .spawn()
            .with_context(|| format!("gagal menjalankan {}", cmd.program.display()))?;
        let job = Job::new()?;
        job.assign(&child)?;
        Ok(Self { child, job })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Hentikan seluruh pohon proses.
    pub fn kill(mut self) {
        self.job.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `std::process::Command` tanpa jendela console.
pub fn std_command(cmd: &Command) -> StdCommand {
    let mut command = StdCommand::new(&cmd.program);
    command.args(&cmd.args).current_dir(&cmd.cwd);
    for (key, value) in &cmd.env {
        command.env(key, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Jalankan perintah singkat (init, stop) dan tunggu hingga `timeout`.
pub fn run_with_timeout(cmd: &Command, log: &std::path::Path, timeout: Duration) -> Result<bool> {
    let mut process = Process::spawn(cmd, log)?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(status) = process.child.try_wait()? {
            return Ok(status.success());
        }
        if std::time::Instant::now() >= deadline {
            process.kill();
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: handle job object boleh dipakai dari thread mana pun.
#[cfg(windows)]
unsafe impl Send for Job {}

#[cfg(windows)]
impl Job {
    fn new() -> Result<Self> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        // SAFETY: argumen null = job tanpa nama dengan security default.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error()).context("gagal membuat job object");
        }
        let job = Job(handle);
        // SAFETY: struct zeroed valid untuk API ini; hanya flag kill-on-close yang diisi.
        unsafe {
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return Err(std::io::Error::last_os_error()).context("gagal mengatur job object");
            }
        }
        Ok(job)
    }

    fn assign(&self, child: &Child) -> Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        // SAFETY: handle proses valid selama `child` hidup.
        let ok = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as _) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error())
                .context("gagal memasukkan proses ke job object");
        }
        Ok(())
    }

    fn terminate(&self) {
        // SAFETY: handle job milik struct ini.
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: handle ditutup sekali; kill-on-close menghentikan proses yang tersisa.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
struct Job;

#[cfg(not(windows))]
impl Job {
    fn new() -> Result<Self> {
        Ok(Job)
    }
    fn assign(&self, _: &Child) -> Result<()> {
        Ok(())
    }
    fn terminate(&self) {}
}
