//! Menjalankan service sebagai proses latar dalam Windows Job Object, sehingga seluruh
//! pohon proses (mis. master + worker nginx) bisa dihentikan bersama dan otomatis ikut
//! mati bila daemon DevLock berhenti.

use std::fs::{File, OpenOptions};
use std::process::{Child, Command as StdCommand, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::services::Command;

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const DETACHED_PROCESS: u32 = 0x0000_0008;
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
pub const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

/// Log service lebih besar dari ini diputar saat service (re)start.
pub const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

/// Bila `log` melebihi `max` byte, ganti namanya menjadi `<log>.1` (menimpa `.1` lama).
/// Dilakukan sebelum file dibuka, karena log yang sedang dipakai proses tidak bisa
/// di-rename di Windows. Kegagalan diabaikan agar service tetap bisa start.
pub fn rotate_log(log: &std::path::Path, max: u64) {
    if std::fs::metadata(log).is_ok_and(|m| m.len() > max) {
        let mut rotated = log.as_os_str().to_owned();
        rotated.push(".1");
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(log, &rotated);
    }
}

pub struct Process {
    pub child: Child,
    job: Job,
}

impl Process {
    pub fn spawn(cmd: &Command, log: &std::path::Path) -> Result<Self> {
        if let Some(dir) = log.parent() {
            std::fs::create_dir_all(dir)?;
        }
        rotate_log(log, MAX_LOG_BYTES);
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

/// Jalankan `command` sebagai proses latar dengan stdin `NUL` dan stdout/stderr ke `log`,
/// lalu kembalikan PID-nya.
///
/// `std::process::Command` di Windows mewariskan *semua* handle inheritable milik
/// pemanggil, termasuk pipe stdout/stderr (`devlock start | Out-String`). Proses latar
/// yang memegang pipe itu membuat pembaca tidak pernah menerima EOF. Di sini pewarisan
/// dibatasi lewat `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` ke handle `NUL` dan `log` saja.
#[cfg(windows)]
pub fn spawn_detached(command: StdCommand, log: File, flags: u32) -> std::io::Result<u32> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{
        CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
    };
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
        PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES,
        STARTUPINFOEXW,
    };

    let null = OpenOptions::new().read(true).write(true).open("NUL")?;
    let handles: [HANDLE; 2] = [
        null.as_raw_handle() as HANDLE,
        log.as_raw_handle() as HANDLE,
    ];
    for handle in handles {
        // SAFETY: handle milik `null`/`log` yang hidup sampai akhir fungsi.
        if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }

    let mut cmdline: Vec<u16> = Vec::new();
    cmdline.push(u16::from(b'"'));
    cmdline.extend(command.get_program().encode_wide());
    cmdline.push(u16::from(b'"'));
    for arg in command.get_args() {
        cmdline.push(u16::from(b' '));
        push_arg(&mut cmdline, arg);
    }
    cmdline.push(0);
    let env = environment_block(&command);
    let cwd: Option<Vec<u16>> = command
        .get_current_dir()
        .map(|dir| dir.as_os_str().encode_wide().chain(Some(0)).collect());

    let mut size = 0usize;
    // SAFETY: panggilan pertama hanya menanyakan ukuran buffer (gagal dengan sengaja).
    unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
    let mut buffer = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    let attributes = buffer.as_mut_ptr().cast();
    // SAFETY: buffer berukuran minimal `size` byte dan sejajar untuk pointer.
    if unsafe { InitializeProcThreadAttributeList(attributes, 1, 0, &mut size) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `handles`, `cmdline`, `env`, dan `cwd` hidup sampai CreateProcessW selesai.
    let result = unsafe {
        let ok = UpdateProcThreadAttribute(
            attributes,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_ptr().cast(),
            std::mem::size_of_val(&handles),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            let mut info: STARTUPINFOEXW = std::mem::zeroed();
            info.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
            info.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            info.StartupInfo.hStdInput = handles[0];
            info.StartupInfo.hStdOutput = handles[1];
            info.StartupInfo.hStdError = handles[1];
            info.lpAttributeList = attributes;
            let mut process: PROCESS_INFORMATION = std::mem::zeroed();
            let ok = CreateProcessW(
                std::ptr::null(),
                cmdline.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                flags | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                env.as_ptr().cast(),
                cwd.as_ref().map_or(std::ptr::null(), |dir| dir.as_ptr()),
                &info.StartupInfo,
                &mut process,
            );
            if ok == 0 {
                Err(std::io::Error::last_os_error())
            } else {
                CloseHandle(process.hThread);
                CloseHandle(process.hProcess);
                Ok(process.dwProcessId)
            }
        }
    };
    // SAFETY: list sudah diinisialisasi di atas.
    unsafe { DeleteProcThreadAttributeList(attributes) };
    result
}

#[cfg(not(windows))]
pub fn spawn_detached(mut command: StdCommand, log: File, _flags: u32) -> std::io::Result<u32> {
    let err = log.try_clone()?;
    command.stdin(Stdio::null()).stdout(log).stderr(err);
    Ok(command.spawn()?.id())
}

/// Kutip satu argumen sesuai aturan parsing command line MSVC (`CommandLineToArgvW`).
#[cfg(windows)]
fn push_arg(cmdline: &mut Vec<u16>, arg: &std::ffi::OsStr) {
    use std::os::windows::ffi::OsStrExt;
    let (quote, backslash) = (u16::from(b'"'), u16::from(b'\\'));
    let arg: Vec<u16> = arg.encode_wide().collect();
    let needs_quotes = arg.is_empty()
        || arg
            .iter()
            .any(|&c| c == u16::from(b' ') || c == u16::from(b'\t') || c == quote);
    if !needs_quotes {
        cmdline.extend(arg);
        return;
    }
    cmdline.push(quote);
    let mut backslashes = 0;
    for c in arg {
        if c == backslash {
            backslashes += 1;
        } else {
            if c == quote {
                cmdline.extend(std::iter::repeat(backslash).take(backslashes + 1));
            }
            backslashes = 0;
        }
        cmdline.push(c);
    }
    cmdline.extend(std::iter::repeat(backslash).take(backslashes));
    cmdline.push(quote);
}

/// Environment proses ini ditambah perubahan `command.env(..)`, dalam format blok Unicode.
#[cfg(windows)]
fn environment_block(command: &StdCommand) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os().collect();
    for (key, value) in command.get_envs() {
        vars.retain(|(k, _)| !k.eq_ignore_ascii_case(key));
        if let Some(value) = value {
            vars.push((key.to_owned(), value.to_owned()));
        }
    }
    vars.sort_by_key(|(key, _)| key.to_ascii_uppercase());
    let mut block = Vec::new();
    for (key, value) in vars {
        block.extend(key.encode_wide());
        block.push(u16::from(b'='));
        block.extend(value.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

/// Jadikan handle stdio proses ini tidak ikut diwariskan ke proses anak. Child yang memang
/// diberi stdio (mis. lewat `Stdio::inherit`) tetap mendapat duplikat dari `std`.
pub fn make_stdio_private() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT};
        use windows_sys::Win32::System::Console::{
            GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        };
        for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: handle stdio milik proses ini; kegagalan (handle kosong) diabaikan.
            unsafe { SetHandleInformation(GetStdHandle(id), HANDLE_FLAG_INHERIT, 0) };
        }
    }
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

#[cfg(test)]
mod rotate_tests {
    use super::rotate_log;

    #[test]
    fn rotates_large_logs_only() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("nginx.log");
        std::fs::write(&log, vec![b'x'; 20]).unwrap();
        rotate_log(&log, 100);
        assert!(log.is_file());
        rotate_log(&log, 10);
        assert!(!log.exists());
        assert_eq!(
            std::fs::read(tmp.path().join("nginx.log.1")).unwrap().len(),
            20
        );
        std::fs::write(&log, vec![b'y'; 30]).unwrap();
        rotate_log(&log, 10);
        assert_eq!(
            std::fs::read(tmp.path().join("nginx.log.1")).unwrap().len(),
            30
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::windows::io::FromRawHandle;
    use std::time::Instant;

    fn quoted(arg: &str) -> String {
        let mut cmdline = Vec::new();
        push_arg(&mut cmdline, std::ffi::OsStr::new(arg));
        String::from_utf16(&cmdline).unwrap()
    }

    #[test]
    fn push_arg_follows_msvc_rules() {
        assert_eq!(quoted("daemon"), "daemon");
        assert_eq!(quoted(""), "\"\"");
        assert_eq!(quoted(r"C:\Program Files\x"), r#""C:\Program Files\x""#);
        assert_eq!(quoted("a b\\"), "\"a b\\\\\"");
        assert_eq!(quoted("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    /// Regresi: pipe milik pemanggil (inheritable) tidak boleh ikut dipegang proses latar.
    #[test]
    fn spawn_detached_does_not_inherit_caller_handles() {
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
        use windows_sys::Win32::System::Pipes::CreatePipe;

        let (mut read, mut write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
        let attrs = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: 1,
        };
        // SAFETY: pointer keluaran valid; kedua ujung pipe inheritable seperti stdout shell.
        assert_ne!(unsafe { CreatePipe(&mut read, &mut write, &attrs, 0) }, 0);

        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("out.log");
        let log = File::create(&log_path).unwrap();
        let mut command = StdCommand::new("cmd.exe");
        // Tulis ke log lalu tetap hidup ~5 detik.
        command
            .args([
                "/c",
                "echo",
                "%DEVLOCK_SPAWN_TEST%&",
                "ping",
                "-n",
                "6",
                "127.0.0.1",
            ])
            .env("DEVLOCK_SPAWN_TEST", "halo-devlock")
            .current_dir(dir.path());
        spawn_detached(command, log, CREATE_NO_WINDOW).unwrap();

        // SAFETY: handle `write` milik test ini dan ditutup sekali.
        unsafe { CloseHandle(write) };
        // SAFETY: `read` dimiliki File mulai sekarang.
        let mut reader = unsafe { File::from_raw_handle(read as _) };
        let started = Instant::now();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "pipe baru EOF setelah {:?}; child mewarisi handle pemanggil",
            started.elapsed()
        );

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let text = std::fs::read_to_string(&log_path).unwrap_or_default();
            if text.contains("halo-devlock") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "log tidak berisi output: {text:?}"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
