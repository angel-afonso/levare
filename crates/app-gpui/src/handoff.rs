//! Handing a launch over to the Levare that is already running.
//!
//! On Windows and Linux the file manager opens a score by starting Levare
//! again with the file in its arguments. The Levare already running holds the
//! autosave, so a second window would have none. Instead, the running Levare
//! listens on a local endpoint, and a new launch that reaches it passes it
//! the files and exits; one that does not starts as before.
//!
//! The endpoint is a Unix domain socket on Linux (and on macOS, where only
//! the tests use it) and a named pipe on Windows, both from the standard
//! library on the client side. Its name comes from the recovery file the
//! running Levare holds, so a launch hands off to exactly the Levare whose
//! autosave it would otherwise be refused, and Levares started with another
//! `LEVARE_DATA_DIR` stay apart. Only the holder of the recovery lock
//! listens, which is what makes it safe to replace a socket a crash left
//! behind.
//!
//! The exchange, once connected:
//!
//! 1. The running Levare greets: [`GREETING`], then its process id (`u32`,
//!    little-endian). Windows lets that process come to the front.
//! 2. The new launch sends the number of paths (`u32`), then each path as its
//!    length (`u32`) and its bytes in the platform's own encoding (UTF-16LE
//!    on Windows), so any name the system allows gets through intact.
//! 3. The running Levare answers [`ACCEPTED`] once the request is queued, or
//!    [`REFUSED`] when it is quitting.
//!
//! Nothing here touches GPUI: `system_open` connects it to the window.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// How long a new launch waits for the running Levare before starting on its
/// own. The answer comes from a thread that does nothing else, so only a
/// Levare that is hung or gone takes this long.
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// The first bytes the running Levare sends; the last one is the protocol's
/// version.
const GREETING: &[u8; 8] = b"LEVARE\0\x01";
const ACCEPTED: &[u8; 2] = b"OK";
const REFUSED: &[u8; 2] = b"NO";

/// Far more than a file manager passes at once; a bound on what a request
/// can make the running Levare allocate.
const MAX_PATHS: u32 = 1024;
const MAX_PATH_BYTES: u32 = 64 * 1024;

/// Where the running Levare listens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    #[cfg(unix)]
    socket: PathBuf,
    #[cfg(windows)]
    pipe: String,
}

impl Address {
    /// The endpoint of the Levare that holds `recovery`. Debug and release
    /// builds get different ones: a development build does not hand its
    /// launch over to the installed Levare, nor the other way round.
    pub fn for_recovery(recovery: &Path) -> Address {
        #[cfg(unix)]
        {
            // The per-user runtime directory (0700) when there is one, the
            // data folder otherwise.
            let directory = dirs::runtime_dir()
                .or_else(|| recovery.parent().map(Path::to_path_buf))
                .unwrap_or_default();
            Address::in_directory(&directory, recovery)
        }
        #[cfg(windows)]
        {
            Address {
                pipe: format!(r"\\.\pipe\levare-{:016x}", key(recovery)),
            }
        }
    }

    /// The socket for `recovery` inside `directory`.
    #[cfg(unix)]
    fn in_directory(directory: &Path, recovery: &Path) -> Address {
        Address {
            socket: directory.join(format!("levare-{:016x}.sock", key(recovery))),
        }
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(unix)]
        {
            write!(f, "{}", self.socket.display())
        }
        #[cfg(windows)]
        {
            f.write_str(&self.pipe)
        }
    }
}

/// A name for the endpoint that stays the same across Levare versions (so
/// not `std`'s hasher, which may change): FNV-1a over the build profile and
/// the recovery path.
fn key(recovery: &Path) -> u64 {
    let profile: &[u8] = if cfg!(debug_assertions) {
        b"debug\0"
    } else {
        b"release\0"
    };
    fnv1a(profile.iter().chain(&path_bytes(recovery)))
}

fn fnv1a<'a>(bytes: impl IntoIterator<Item = &'a u8>) -> u64 {
    bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// What became of a launch's attempt to hand itself over.
#[derive(Debug)]
pub enum Outcome {
    /// The running Levare took the files; this launch should exit.
    HandedOff,
    /// No Levare listens there, or one crashed and left its socket behind.
    NoneRunning,
    /// A Levare answered but did not take the files: it is quitting, hung,
    /// or speaks another version of the exchange.
    Failed(io::Error),
}

/// Offers `paths` (absolute; possibly none, to bring the window forward) to
/// the Levare listening at `address`, giving up after `timeout`.
pub fn hand_off(address: &Address, paths: &[PathBuf], timeout: Duration) -> Outcome {
    let address = address.clone();
    let paths = paths.to_vec();
    let (sender, receiver) = mpsc::channel();
    // A thread, because a named pipe has no read timeout: if the other end
    // never answers, this one is left blocked and the process carries on.
    let spawned = std::thread::Builder::new()
        .name("levare-handoff".into())
        .spawn(move || {
            let _ = sender.send(exchange(&address, &paths, timeout));
        });
    if let Err(error) = spawned {
        return Outcome::Failed(error);
    }
    match receiver.recv_timeout(timeout) {
        Ok(outcome) => outcome,
        Err(_) => Outcome::Failed(io::Error::new(
            io::ErrorKind::TimedOut,
            "the running Levare did not answer",
        )),
    }
}

fn exchange(address: &Address, paths: &[PathBuf], timeout: Duration) -> Outcome {
    let mut stream = match transport::connect(address, timeout) {
        Ok(stream) => stream,
        Err(error) if nobody_listens(&error) => return Outcome::NoneRunning,
        Err(error) => return Outcome::Failed(error),
    };
    match request(&mut stream, paths) {
        Ok(()) => Outcome::HandedOff,
        Err(error) => Outcome::Failed(error),
    }
}

/// The errors that mean no Levare is there to answer.
fn nobody_listens(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

/// The new launch's side of the exchange.
fn request(stream: &mut (impl Read + Write), paths: &[PathBuf]) -> io::Result<()> {
    let pid = read_greeting(stream)?;
    transport::allow_to_front(pid);
    write_paths(stream, paths)?;
    stream.flush()?;
    let mut answer = [0; 2];
    stream.read_exact(&mut answer)?;
    match &answer {
        ACCEPTED => Ok(()),
        REFUSED => Err(io::Error::other("the running Levare is quitting")),
        _ => Err(invalid("unexpected answer")),
    }
}

/// The running Levare's side of the exchange. `take` queues the paths and
/// says whether it could.
fn answer(
    stream: &mut (impl Read + Write),
    closing: &AtomicBool,
    take: &(dyn Fn(Vec<PathBuf>) -> bool + Send + Sync),
) -> io::Result<()> {
    stream.write_all(GREETING)?;
    stream.write_all(&std::process::id().to_le_bytes())?;
    stream.flush()?;
    let paths = read_paths(stream)?;
    let taken = !closing.load(Ordering::SeqCst) && take(paths);
    stream.write_all(if taken { ACCEPTED } else { REFUSED })?;
    stream.flush()
}

fn read_greeting(stream: &mut impl Read) -> io::Result<u32> {
    let mut greeting = [0; GREETING.len()];
    stream.read_exact(&mut greeting)?;
    if &greeting != GREETING {
        return Err(invalid("not a Levare, or another version of Levare"));
    }
    read_u32(stream)
}

fn write_paths(stream: &mut impl Write, paths: &[PathBuf]) -> io::Result<()> {
    let count = u32::try_from(paths.len()).map_err(|_| invalid("too many files"))?;
    let mut message = count.to_le_bytes().to_vec();
    for path in paths {
        let bytes = path_bytes(path);
        let length = u32::try_from(bytes.len()).map_err(|_| invalid("path too long"))?;
        message.extend_from_slice(&length.to_le_bytes());
        message.extend_from_slice(&bytes);
    }
    stream.write_all(&message)
}

fn read_paths(stream: &mut impl Read) -> io::Result<Vec<PathBuf>> {
    let count = read_u32(stream)?;
    if count > MAX_PATHS {
        return Err(invalid("too many files"));
    }
    let mut paths = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let length = read_u32(stream)?;
        if length > MAX_PATH_BYTES {
            return Err(invalid("path too long"));
        }
        let mut bytes = vec![0; length as usize];
        stream.read_exact(&mut bytes)?;
        paths.push(path_from_bytes(bytes)?);
    }
    Ok(paths)
}

fn read_u32(stream: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    stream.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path_from_bytes(bytes: Vec<u8>) -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    let (pairs, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return Err(invalid("odd UTF-16 path"));
    }
    let wide: Vec<u16> = pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    Ok(PathBuf::from(OsString::from_wide(&wide)))
}

/// The running Levare's endpoint. It serves on its own threads until
/// [`Listener::close`] or the end of the process.
pub struct Listener {
    closing: Arc<AtomicBool>,
    #[cfg(unix)]
    socket: PathBuf,
}

impl Listener {
    /// Starts listening at `address`. Call it only while holding the
    /// recovery lock `address` was made from: a socket already there is then
    /// a crashed Levare's, and is replaced. `take` runs on a listener thread
    /// with each request's paths and says whether they were queued.
    pub fn start(
        address: &Address,
        take: impl Fn(Vec<PathBuf>) -> bool + Send + Sync + 'static,
    ) -> io::Result<Listener> {
        let closing = Arc::new(AtomicBool::new(false));
        transport::serve(address, closing.clone(), Arc::new(take))?;
        Ok(Listener {
            closing,
            #[cfg(unix)]
            socket: address.socket.clone(),
        })
    }

    /// Stops taking files, for a Levare that is quitting: a launch that
    /// reaches it from now on starts on its own instead of handing over
    /// files that would never open.
    pub fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.socket);
        }
    }
}

type Take = Arc<dyn Fn(Vec<PathBuf>) -> bool + Send + Sync>;

#[cfg(unix)]
mod transport {
    use super::*;
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};

    pub fn connect(address: &Address, timeout: Duration) -> io::Result<UnixStream> {
        let stream = UnixStream::connect(&address.socket)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(stream)
    }

    /// Only Windows keeps a background process from taking the focus.
    pub fn allow_to_front(_pid: u32) {}

    pub fn serve(address: &Address, closing: Arc<AtomicBool>, take: Take) -> io::Result<()> {
        let socket = &address.socket;
        match std::fs::symlink_metadata(socket) {
            // Left by a Levare that crashed: nobody else can be listening
            // while this process holds the recovery lock.
            Ok(found) if found.file_type().is_socket() => std::fs::remove_file(socket)?,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "something that is not a socket is in the way",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(socket)?;
        // Connecting takes write permission: only this user may hand files
        // over. The runtime directory is already private; the data folder
        // may not be.
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
        std::thread::Builder::new()
            .name("levare-handoff-listener".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    // A client that stalls holds this thread up to the
                    // timeout, never for good.
                    if stream.set_read_timeout(Some(TIMEOUT)).is_err()
                        || stream.set_write_timeout(Some(TIMEOUT)).is_err()
                    {
                        continue;
                    }
                    let _ = answer(&mut stream, &closing, &*take);
                }
            })?;
        Ok(())
    }
}

#[cfg(windows)]
mod transport {
    use super::*;
    use std::fs::File;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::time::Instant;
    use windows::Win32::Foundation::{ERROR_PIPE_CONNECTED, HANDLE};
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows::core::{HRESULT, PCWSTR};

    /// All of the pipe's instances are taken: another launch is handing
    /// over, or the listener is between two connections.
    const ERROR_PIPE_BUSY: i32 = 231;
    /// The server may only identify this process, not act as it.
    const SECURITY_IDENTIFICATION: u32 = 1 << 16;

    pub fn connect(address: &Address, timeout: Duration) -> io::Result<File> {
        let deadline = Instant::now() + timeout;
        loop {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .security_qos_flags(SECURITY_IDENTIFICATION)
                .open(&address.pipe)
            {
                Err(error)
                    if error.raw_os_error() == Some(ERROR_PIPE_BUSY)
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                result => return result,
            }
        }
    }

    /// Lets the running Levare bring its window to the front: Windows only
    /// allows that to the process the musician just started, and to whom it
    /// passes the right.
    pub fn allow_to_front(pid: u32) {
        let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid) };
    }

    pub fn serve(address: &Address, closing: Arc<AtomicBool>, take: Take) -> io::Result<()> {
        let name: Vec<u16> = address.pipe.encode_utf16().chain([0]).collect();
        // The first instance fails if the name is taken, by another Levare
        // or by anything else.
        let mut waiting = instance(&name, true)?;
        std::thread::Builder::new()
            .name("levare-handoff-listener".into())
            .spawn(move || {
                loop {
                    let connected =
                        unsafe { ConnectNamedPipe(HANDLE(waiting.as_raw_handle()), None) };
                    let connected = match connected {
                        Ok(()) => true,
                        Err(error) => error.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0),
                    };
                    // The next instance is ready before this one is served,
                    // so a launch always finds the pipe.
                    let Ok(next) = instance(&name, false) else {
                        return;
                    };
                    let current = std::mem::replace(&mut waiting, next);
                    if !connected {
                        continue;
                    }
                    let closing = closing.clone();
                    let take = take.clone();
                    // Its own thread: a pipe has no read timeout, and a client
                    // that stalls must not keep the next one waiting.
                    let _ = std::thread::Builder::new()
                        .name("levare-handoff".into())
                        .spawn(move || {
                            let mut stream = File::from(current);
                            let _ = answer(&mut stream, &closing, &*take);
                        });
                }
            })?;
        Ok(())
    }

    /// One instance of the pipe, for one connection. Its default security
    /// lets other users only read, so only this user can hand files over;
    /// remote clients are refused.
    fn instance(name: &[u16], first: bool) -> io::Result<OwnedHandle> {
        let mut mode = PIPE_ACCESS_DUPLEX;
        if first {
            mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                4096,
                4096,
                0,
                None,
            )
        };
        if handle.is_invalid() {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(handle.0) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// One end of a conversation: reads what the other end said, keeps what
    /// this end writes.
    struct Scripted {
        incoming: io::Cursor<Vec<u8>>,
        outgoing: Vec<u8>,
    }

    impl Scripted {
        fn new(incoming: Vec<u8>) -> Scripted {
            Scripted {
                incoming: io::Cursor::new(incoming),
                outgoing: Vec::new(),
            }
        }
    }

    impl Read for Scripted {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.incoming.read(buf)
        }
    }

    impl Write for Scripted {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.outgoing.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn scores() -> Vec<PathBuf> {
        let home = std::env::temp_dir();
        vec![
            home.join("Mi obra nº 1 — Ñandú.levare"),
            home.join("coral a 4 voces").join("ラ・カンパネラ.musicxml"),
        ]
    }

    fn greeting(pid: u32) -> Vec<u8> {
        let mut bytes = GREETING.to_vec();
        bytes.extend_from_slice(&pid.to_le_bytes());
        bytes
    }

    #[test]
    fn paths_cross_intact_with_spaces_and_any_alphabet() {
        for paths in [scores(), Vec::new()] {
            let mut written = Vec::new();
            write_paths(&mut written, &paths).unwrap();
            assert_eq!(read_paths(&mut written.as_slice()).unwrap(), paths);
        }
    }

    #[test]
    fn the_new_launch_greets_sends_the_paths_and_reads_the_answer() {
        let mut incoming = greeting(4321);
        incoming.extend_from_slice(ACCEPTED);
        let mut stream = Scripted::new(incoming);
        request(&mut stream, &scores()).unwrap();
        assert_eq!(
            read_paths(&mut stream.outgoing.as_slice()).unwrap(),
            scores()
        );

        let mut incoming = greeting(4321);
        incoming.extend_from_slice(REFUSED);
        assert!(request(&mut Scripted::new(incoming), &scores()).is_err());
    }

    #[test]
    fn something_else_on_the_endpoint_is_not_taken_for_levare() {
        let mut other_version = greeting(1);
        other_version[GREETING.len() - 1] = 2;
        other_version.extend_from_slice(ACCEPTED);
        for incoming in [b"HTTP/1.1 400\r\n".to_vec(), other_version, Vec::new()] {
            assert!(request(&mut Scripted::new(incoming), &[]).is_err());
        }
    }

    #[test]
    fn the_running_levare_queues_the_paths_unless_it_is_quitting() {
        let mut incoming = Vec::new();
        write_paths(&mut incoming, &scores()).unwrap();
        let received = Mutex::new(Vec::new());
        let take = |paths| {
            received.lock().unwrap().push(paths);
            true
        };

        let mut stream = Scripted::new(incoming.clone());
        answer(&mut stream, &AtomicBool::new(false), &take).unwrap();
        let mut said = stream.outgoing.as_slice();
        assert_eq!(read_greeting(&mut said).unwrap(), std::process::id());
        assert_eq!(said, ACCEPTED);
        assert_eq!(*received.lock().unwrap(), vec![scores()]);

        let mut stream = Scripted::new(incoming.clone());
        answer(&mut stream, &AtomicBool::new(true), &take).unwrap();
        assert!(stream.outgoing.ends_with(REFUSED));
        assert_eq!(received.lock().unwrap().len(), 1);

        let mut stream = Scripted::new(incoming);
        answer(&mut stream, &AtomicBool::new(false), &|_| false).unwrap();
        assert!(stream.outgoing.ends_with(REFUSED));
    }

    #[test]
    fn a_request_cannot_make_the_running_levare_allocate_without_bound() {
        let mut too_many = Vec::new();
        too_many.extend_from_slice(&(MAX_PATHS + 1).to_le_bytes());
        assert!(read_paths(&mut too_many.as_slice()).is_err());

        let mut too_long = 1u32.to_le_bytes().to_vec();
        too_long.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_paths(&mut too_long.as_slice()).is_err());

        let mut truncated = 1u32.to_le_bytes().to_vec();
        truncated.extend_from_slice(&10u32.to_le_bytes());
        truncated.extend_from_slice(b"abc");
        assert!(read_paths(&mut truncated.as_slice()).is_err());
    }

    #[test]
    fn each_recovery_file_has_its_own_stable_endpoint() {
        // FNV-1a's published value for no input: the name must not change
        // between Levare versions, or a new one could not reach an old one.
        assert_eq!(fnv1a(&[]), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        let base = std::env::temp_dir();
        let one = base.join("Levare").join("recovery.partitura");
        let other = base.join("Levare QA").join("recovery.partitura");
        assert_eq!(Address::for_recovery(&one), Address::for_recovery(&one));
        assert_ne!(Address::for_recovery(&one), Address::for_recovery(&other));
    }

    /// A recovery path of its own for each test, so tests running at once
    /// on the same machine never reach each other's listener.
    fn test_address(directory: &Path) -> Address {
        let recovery = directory.join("recovery.partitura");
        #[cfg(unix)]
        {
            Address::in_directory(directory, &recovery)
        }
        #[cfg(windows)]
        {
            Address::for_recovery(&recovery)
        }
    }

    fn listen(address: &Address) -> (Listener, mpsc::Receiver<Vec<PathBuf>>) {
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        let listener = Listener::start(address, move |paths| {
            sender.lock().unwrap().send(paths).is_ok()
        })
        .unwrap();
        (listener, receiver)
    }

    #[test]
    fn a_second_launch_hands_its_files_to_the_running_levare() {
        let directory = tempfile::tempdir().unwrap();
        let address = test_address(directory.path());
        let (_listener, received) = listen(&address);

        assert!(matches!(
            hand_off(&address, &scores(), TIMEOUT),
            Outcome::HandedOff
        ));
        assert_eq!(received.recv_timeout(TIMEOUT).unwrap(), scores());
        // Without files it only asks for the window.
        assert!(matches!(
            hand_off(&address, &[], TIMEOUT),
            Outcome::HandedOff
        ));
        assert_eq!(
            received.recv_timeout(TIMEOUT).unwrap(),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn with_no_levare_running_the_launch_starts_on_its_own() {
        let directory = tempfile::tempdir().unwrap();
        assert!(matches!(
            hand_off(&test_address(directory.path()), &scores(), TIMEOUT),
            Outcome::NoneRunning
        ));
    }

    #[test]
    fn a_levare_that_is_quitting_turns_the_files_away() {
        let directory = tempfile::tempdir().unwrap();
        let address = test_address(directory.path());
        let (listener, received) = listen(&address);
        listener.close();
        assert!(!matches!(
            hand_off(&address, &scores(), TIMEOUT),
            Outcome::HandedOff
        ));
        assert!(received.try_recv().is_err());
    }

    #[test]
    fn a_hung_levare_does_not_hold_the_launch_up() {
        let directory = tempfile::tempdir().unwrap();
        let address = test_address(directory.path());
        let _listener = Listener::start(&address, |_| {
            std::thread::sleep(Duration::from_secs(3));
            true
        })
        .unwrap();
        let started = std::time::Instant::now();
        let outcome = hand_off(&address, &scores(), Duration::from_millis(300));
        assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    /// A crash leaves the socket file behind: the next launch finds nobody
    /// there and starts, and its listener replaces the file.
    #[cfg(unix)]
    #[test]
    fn a_socket_left_by_a_crash_is_taken_over() {
        let directory = tempfile::tempdir().unwrap();
        let address = test_address(directory.path());
        drop(std::os::unix::net::UnixListener::bind(&address.socket).unwrap());
        assert!(address.socket.exists());
        assert!(matches!(
            hand_off(&address, &scores(), TIMEOUT),
            Outcome::NoneRunning
        ));

        let (_listener, received) = listen(&address);
        assert!(matches!(
            hand_off(&address, &scores(), TIMEOUT),
            Outcome::HandedOff
        ));
        assert_eq!(received.recv_timeout(TIMEOUT).unwrap(), scores());
    }

    #[cfg(unix)]
    #[test]
    fn only_this_user_can_reach_the_socket_and_files_are_not_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let address = test_address(directory.path());
        let (listener, _received) = listen(&address);
        let mode = std::fs::metadata(&address.socket)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        listener.close();
        assert!(!address.socket.exists());

        std::fs::write(&address.socket, "no es un socket").unwrap();
        assert!(Listener::start(&address, |_| true).is_err());
        assert!(address.socket.is_file());
    }

    /// Only one Levare may own the pipe's name.
    #[cfg(windows)]
    #[test]
    fn a_second_listener_cannot_take_the_pipe() {
        let directory = tempfile::tempdir().unwrap();
        let address = test_address(directory.path());
        let (_listener, _received) = listen(&address);
        assert!(Listener::start(&address, |_| true).is_err());
    }
}
