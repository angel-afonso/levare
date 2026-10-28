//! Ends the agent's whole process tree with its session on Windows.
//!
//! `AcpAgent` starts the agent in a process group of its own and kills the
//! group on Unix, but on Windows it kills only the process it started. For
//! `npx.cmd` that is `cmd.exe`, and the `node` and agent behind it go on
//! running. [`JobAgent`] starts the agent through the same
//! `AcpAgent::spawn_process`, puts it in a Job Object that kills every
//! process in it once Levare lets go of it, and talks to it over its stdio
//! as `AcpAgent` does.
//!
//! Off Windows it is only built for its tests, without the job.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::{self as acp, AcpAgent, Agent, ByteStreams, Client, ConnectTo};
use futures::AsyncReadExt;
use futures::future::{self, Either};

/// How much of the end of stderr a failure reports, as `AcpAgent` does.
const STDERR_LIMIT: usize = 64 * 1024;
/// How long the protocol, or stderr, may take to finish once the process
/// has exited.
const EXIT_GRACE: Duration = Duration::from_secs(1);

/// An agent whose processes all end with the connection.
pub(crate) struct JobAgent(pub AcpAgent);

impl ConnectTo<Client> for JobAgent {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), acp::Error> {
        let (stdin, stdout, mut stderr, mut child) = self.0.spawn_process()?;
        // At once: a process the launcher starts before this is left out of
        // the job.
        let job = Job::new().and_then(|job| job.assign(&child).map(|()| job));
        let exited = child.status();
        // However the connection ends. Without a job, at least the process
        // Levare started goes, as with `AcpAgent`.
        let _processes = OnDrop::new(move || {
            let _ = child.kill();
            drop(job);
        });

        let tail = Arc::new(Mutex::new(Vec::new()));
        let drained = tokio::spawn({
            let tail = tail.clone();
            async move {
                let mut buffer = [0u8; 8192];
                while let Ok(read @ 1..) = stderr.read(&mut buffer).await {
                    let mut tail = tail.lock().unwrap();
                    tail.extend_from_slice(&buffer[..read]);
                    let excess = tail.len().saturating_sub(STDERR_LIMIT);
                    tail.drain(..excess);
                }
            }
        });
        let abort = drained.abort_handle();
        let _drained = OnDrop::new(move || abort.abort());

        let protocol = Box::pin(ConnectTo::<Client>::connect_to(
            ByteStreams::new(stdin, stdout),
            client,
        ));
        let (status, protocol) = match future::select(protocol, Box::pin(exited)).await {
            Either::Left((result, exited)) => {
                result?;
                // The agent may have closed its stdout by exiting: if so,
                // how it exited is what to report.
                let Ok(status) = tokio::time::timeout(EXIT_GRACE, exited).await else {
                    return Ok(());
                };
                (status, None)
            }
            Either::Right((status, protocol)) => (status, Some(protocol)),
        };
        let status = status.map_err(|error| {
            acp::util::internal_error(format!("Failed to wait for process: {error}"))
        })?;
        if status.success() {
            // The protocol may still be reading what the agent wrote last.
            return match protocol {
                Some(protocol) => tokio::time::timeout(EXIT_GRACE, protocol)
                    .await
                    .unwrap_or(Ok(())),
                None => Ok(()),
            };
        }
        let _ = tokio::time::timeout(EXIT_GRACE, drained).await;
        let stderr = String::from_utf8_lossy(&tail.lock().unwrap())
            .trim()
            .to_string();
        let message = if stderr.is_empty() {
            format!("Process exited with {status}")
        } else {
            format!("Process exited with {status}: {stderr}")
        };
        Err(acp::util::internal_error(message))
    }
}

/// Runs a closure when dropped.
struct OnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> OnDrop<F> {
    fn new(run: F) -> Self {
        Self(Some(run))
    }
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        if let Some(run) = self.0.take() {
            run();
        }
    }
}

/// A Job Object that kills every process in it when it is closed. The
/// processes a member starts join it too.
#[cfg(windows)]
pub(crate) struct Job(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: a job's handle may be used and closed from any thread.
#[cfg(windows)]
unsafe impl Send for Job {}

#[cfg(windows)]
impl Job {
    pub(crate) fn new() -> io::Result<Job> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_LIMIT_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectExtendedLimitInformation, SetInformationJobObject,
        };
        // SAFETY: an unnamed job with default security; both may be null.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Job(handle);
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: `limits` is the structure this information class takes,
        // and outlives the call.
        let set = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of_val(&limits) as u32,
            )
        };
        if set == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    pub(crate) fn assign(
        &self,
        process: &impl std::os::windows::io::AsRawHandle,
    ) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        // SAFETY: both handles are open: the job's is ours, and the
        // process's lives as long as `process`.
        let assigned = unsafe { AssignProcessToJobObject(self.0, process.as_raw_handle()) };
        if assigned == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// The processes in the job now.
    #[cfg(test)]
    pub(crate) fn process_ids(&self) -> Vec<usize> {
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicProcessIdList, QueryInformationJobObject,
        };
        /// `JOBOBJECT_BASIC_PROCESS_ID_LIST` with room for 32 ids.
        #[repr(C)]
        struct ProcessIds {
            assigned: u32,
            listed: u32,
            ids: [usize; 32],
        }
        let mut list = ProcessIds {
            assigned: 0,
            listed: 0,
            ids: [0; 32],
        };
        // SAFETY: `list` has the layout this information class writes, and
        // its size is passed along.
        let queried = unsafe {
            QueryInformationJobObject(
                self.0,
                JobObjectBasicProcessIdList,
                (&raw mut list).cast(),
                size_of_val(&list) as u32,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(queried, 0, "{}", io::Error::last_os_error());
        list.ids[..list.listed as usize].to_vec()
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is open and closed only here.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

/// Off Windows, for the tests: no job, so only the process Levare
/// started is killed.
#[cfg(not(windows))]
struct Job;

#[cfg(not(windows))]
impl Job {
    fn new() -> io::Result<Job> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn assign<T>(&self, _process: &T) -> io::Result<()> {
        Ok(())
    }
}
