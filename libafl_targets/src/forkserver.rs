//! Forkserver logic into targets

use std::{
    os::fd::{AsFd, AsRawFd, BorrowedFd},
    sync::OnceLock,
};

use core::sync::atomic::{AtomicBool, Ordering};

use libafl::{
    Error,
    executors::forkserver::{
        FORKSRV_FD, FS_ERROR_SHM_OPEN, FS_NEW_OPT_AUTODTCT, FS_NEW_OPT_MAPSIZE,
        FS_NEW_OPT_SHDMEM_FUZZ, FS_NEW_VERSION_MAX, FS_OPT_ERROR, SHM_ENV_VAR, SHM_FUZZ_ENV_VAR,
    },
};
use libafl_bolts::os::{ChildHandle, ForkResult};

use nix::{
    sys::signal::{SigHandler, Signal},
    unistd::Pid,
};

use crate::coverage::{__afl_map_size, EDGES_MAP_PTR, INPUT_LENGTH_PTR, INPUT_PTR, SHM_FUZZING};
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
use crate::coverage::{__token_start, __token_stop};

/// SAFETY:
///
/// This fd will be closed after being forked as a child. Thus this fd shall never be
/// used after that.
const FORKSRV_R_FD: BorrowedFd<'static> = unsafe { BorrowedFd::borrow_raw(FORKSRV_FD) };
/// SAFETY:
///
/// This fd will be closed after being forked as a child. Thus this fd shall never be
/// used after that.
const FORKSRV_W_FD: BorrowedFd<'static> = unsafe { BorrowedFd::borrow_raw(FORKSRV_FD + 1) };

fn fs_opt_set_error(error: i32) -> i32 {
    (error & 0xFFFF) << 8
}

fn write_to_forkserver(message: &[u8]) -> Result<(), Error> {
    let bytes_written = nix::unistd::write(FORKSRV_W_FD, message)?;
    if bytes_written != message.len() {
        return Err(Error::illegal_state(format!(
            "Could not write to target fd. Expected {} bytes, wrote {bytes_written} bytes",
            message.len()
        )));
    }
    Ok(())
}
fn write_all_to_forkserver(message: &[u8]) -> Result<(), Error> {
    let mut remain_len = message.len();
    while remain_len > 0 {
        let bytes_written = nix::unistd::write(FORKSRV_W_FD, message)?;
        remain_len -= bytes_written;
    }
    Ok(())
}
fn write_u32_to_forkserver(message: u32) -> Result<(), Error> {
    write_to_forkserver(&message.to_ne_bytes())
}
fn write_error_to_forkserver(error: i32) -> Result<(), Error> {
    if error == 0 || error > 0xFFFF {
        return Err(Error::illegal_argument("illegal error sent to forkserver"));
    }
    #[expect(clippy::cast_sign_loss)]
    write_u32_to_forkserver((fs_opt_set_error(error) | FS_OPT_ERROR) as u32)
}

fn read_from_forkserver(message: &mut [u8]) -> Result<(), Error> {
    let bytes_read = nix::unistd::read(FORKSRV_R_FD.as_fd().as_raw_fd(), message)?;
    if bytes_read != message.len() {
        return Err(Error::illegal_state(format!(
            "Could not read from st pipe. Expected {} bytes, got {bytes_read} bytes",
            message.len()
        )));
    }
    Ok(())
}
fn read_u32_from_forkserver() -> Result<u32, Error> {
    let mut buf = [0u8; 4];
    read_from_forkserver(&mut buf)?;
    Ok(u32::from_ne_bytes(buf))
}

/// Guard [`map_shared_memory`] is invoked only once
static SHM_MAP_GUARD: OnceLock<()> = OnceLock::new();

/// Map a shared memory region for the edge coverage map.
/// The [`EDGES_MAP_PTR`] will be updated.
///
/// If anything failed, the forkserver will be notified with
/// [`FS_ERROR_SHM_OPEN`].
pub fn map_shared_memory() -> Result<(), Error> {
    if SHM_MAP_GUARD.set(()).is_err() {
        return Err(Error::illegal_state("shared memory has been mapped before"));
    }
    map_shared_memory_internal()
}

fn map_shared_memory_internal() -> Result<(), Error> {
    let Ok(id_str) = std::env::var(SHM_ENV_VAR) else {
        write_error_to_forkserver(FS_ERROR_SHM_OPEN)?;
        return Err(Error::illegal_argument(
            "Error: variable for edge coverage shared memory is not set",
        ));
    };
    let Ok(shm_id) = id_str.parse() else {
        write_error_to_forkserver(FS_ERROR_SHM_OPEN)?;
        return Err(Error::illegal_argument("Invalid __AFL_SHM_ID value"));
    };
    let map = unsafe { libc::shmat(shm_id, core::ptr::null(), 0) };
    if map.is_null() || core::ptr::eq(map, libc::MAP_FAILED) {
        write_error_to_forkserver(FS_ERROR_SHM_OPEN)?;
        return Err(Error::illegal_state("shmat for map"));
    }
    unsafe {
        EDGES_MAP_PTR = map.cast();
    }
    Ok(())
}

/// Guard [`map_input_shared_memory`] is invoked only once
static INPUT_SHM_MAP_GUARD: OnceLock<()> = OnceLock::new();

/// Map the input shared memory region.
/// The [`INPUT_LENGTH_PTR`] and [`INPUT_PTR`] will be updated.
///
/// If anything failed, the forkserver will be notified with
/// [`FS_ERROR_SHM_OPEN`].
pub fn map_input_shared_memory() -> Result<(), Error> {
    if INPUT_SHM_MAP_GUARD.set(()).is_err() {
        return Err(Error::illegal_state("shared memory has been mapped before"));
    }
    map_input_shared_memory_internal()
}

fn map_input_shared_memory_internal() -> Result<(), Error> {
    let Ok(id_str) = std::env::var(SHM_FUZZ_ENV_VAR) else {
        write_error_to_forkserver(FS_ERROR_SHM_OPEN)?;
        return Err(Error::illegal_argument(
            "Error: variable for fuzzing shared memory is not set",
        ));
    };
    let Ok(shm_id) = id_str.parse() else {
        write_error_to_forkserver(FS_ERROR_SHM_OPEN)?;
        return Err(Error::illegal_argument("Invalid __AFL_SHM_FUZZ_ID value"));
    };
    let map = unsafe { libc::shmat(shm_id, core::ptr::null(), 0) };
    if map.is_null() || core::ptr::eq(map, libc::MAP_FAILED) {
        write_error_to_forkserver(FS_ERROR_SHM_OPEN)?;
        return Err(Error::illegal_state(
            "Could not access fuzzing shared memory",
        ));
    }
    let map: *mut u32 = map.cast();
    unsafe {
        INPUT_LENGTH_PTR = map;
        INPUT_PTR = map.add(1).cast();
    }
    Ok(())
}

/// Parent to handle all logics with forkserver children
pub trait ForkserverParent {
    /// Conduct initializing routine before fuzzing loop.
    ///
    /// Usually, several signal handlers are registered in this function.
    fn pre_fuzzing(&mut self) -> Result<(), Error>;

    /// Spawn a child after the forkserver is ready.
    ///
    /// If the forkserver has killed previous child, `was_killed` will be
    /// set `true`.
    ///
    /// The actual forking should be conduct in this function, and in persistent mode,
    /// some tricks can be done to "fool" the forkserver that a child has been spawned.
    fn spawn_child(&mut self, was_killed: bool) -> Result<ForkResult, Error>;

    /// Interact with spawned child until the child has done its part.
    ///
    /// This function should return a status indicating the status of child. Usually,
    /// that status is determined by `waitpid`.
    fn handle_child_requests(&mut self) -> Result<i32, Error>;
}

/// Whether the forkserver loop is going to stop soon.
///
/// This will be set to true if user send SIGTERM.
static STOP_SOON: AtomicBool = AtomicBool::new(false);

/// Set [`STOP_SOON`] to be `true`. Then the forkserver parent will kill all children
/// and then exit asynchrously.
extern "C" fn std_handle_sigterm(_signal: libc::c_int) {
    STOP_SOON.store(true, Ordering::Relaxed);
}

/// Forkserver parent that can handle both non-persistent and persistent mode
#[derive(Debug, Default)]
pub struct MaybePersistentForkserverParent {
    last_child_pid: Option<i32>,
    /// This field is only touched for persistent mode to indicating
    /// whether the child is temporarily stopped or terminated
    child_stopped: bool,
    old_sigchld_handler: Option<SigHandler>,
    old_sigterm_handler: Option<SigHandler>,
}

impl MaybePersistentForkserverParent {
    /// Create a new forkserver parent.
    #[must_use]
    pub fn new() -> Self {
        MaybePersistentForkserverParent::default()
    }
}

impl ForkserverParent for MaybePersistentForkserverParent {
    fn pre_fuzzing(&mut self) -> Result<(), Error> {
        let old_sigchld_handler =
            (unsafe { nix::sys::signal::signal(Signal::SIGCHLD, SigHandler::SigDfl) })
                .inspect_err(|_| {
                    log::error!("Fail to swap signal handler for SIGCHLD.");
                })?;
        self.old_sigchld_handler = Some(old_sigchld_handler);
        let old_sigterm_handler = (unsafe {
            nix::sys::signal::signal(Signal::SIGTERM, SigHandler::Handler(std_handle_sigterm))
        })
        .inspect_err(|_| {
            log::error!("Fail to swap signal handler for SIGTERM.");
        })?;
        self.old_sigterm_handler = Some(old_sigterm_handler);

        Ok(())
    }

    fn spawn_child(&mut self, was_killed: bool) -> Result<ForkResult, Error> {
        if STOP_SOON.load(Ordering::Relaxed) {
            if let Some(child_pid) = self.last_child_pid.take() {
                nix::sys::signal::kill(Pid::from_raw(child_pid), Signal::SIGKILL)?;
            }
            std::process::exit(0);
        }
        // If we stopped the child in persistent mode, but there was a race
        // condition and afl-fuzz already issued SIGKILL, write off the old
        // process.
        if self.child_stopped && was_killed {
            self.child_stopped = false;
            // unwrap here: child_stopped is set as true only if it has spawned
            // a child, wait it, and get a stopped signal. Moreover, was_killed is
            // true only if the forkserver killed such child. In all cases, the
            // last_child_pid will never be None.
            if nix::sys::wait::waitpid(
                Pid::from_raw(self.last_child_pid.take().unwrap()),
                None,
            )
            .is_err()
            {
                return Err(Error::illegal_state("child_stopped && was_killed"));
            }
        }

        if self.child_stopped {
            // Special handling for persistent mode: if the child is alive but
            // currently stopped, simply restart it with SIGCONT.
    
            // unwrap here: child_stopped is true only if last_child_pid is some.
            let child_pid = *self.last_child_pid.as_ref().unwrap();
            nix::sys::signal::kill(Pid::from_raw(child_pid), Signal::SIGCONT)?;
            self.child_stopped = false;
            Ok(ForkResult::Parent(ChildHandle { pid: child_pid }))
        } else {
            // Once woken up, create a clone of our process.
            let fork_result = (unsafe { libafl_bolts::os::fork() }).inspect_err(|_| {
                log::error!("fork");
            })?;
            match &fork_result {
                ForkResult::Parent(child_pid) => {
                    self.last_child_pid = Some(child_pid.pid);
                }
                ForkResult::Child => unsafe {
                    // unwrap here: the field is assigned in `pre_fuzzing`
                    nix::sys::signal::signal(Signal::SIGCHLD, self.old_sigchld_handler.take().unwrap())
                        .inspect_err(|_| {
                            log::error!("Fail to restore signal handler for SIGCHLD.");
                        })?;
                    // unwrap here: the field is assigned in `pre_fuzzing`
                    nix::sys::signal::signal(Signal::SIGTERM, self.old_sigterm_handler.take().unwrap())
                        .inspect_err(|_| {
                            log::error!("Fail to restore signal handler for SIGTERM.");
                        })?;
                },
            }
            Ok(fork_result)
        }
    }

    fn handle_child_requests(&mut self) -> Result<i32, Error> {
        let mut status = 0i32;
        // unwrap here: the field is assigned if we are parent process in `spawn_child`
        if unsafe { libc::waitpid(*self.last_child_pid.as_ref().unwrap(), &raw mut status, 0) < 0 } {
            return Err(Error::illegal_state("waitpid"));
        }
        if libc::WIFSTOPPED(status) {
            self.child_stopped = true;
        }
        Ok(status)
    }
}

/// Success state when [`start_forkserver`] returned.
#[derive(Debug)]
pub enum ForkserverState {
    /// There is no AFL forkserver responded. In such case,
    /// we should allow user to do a normal execution.
    NoAfl,
    /// Current process is a spawned child.
    Child,
}

/// Guard [`start_forkserver`] is invoked only once
static FORKSERVER_GUARD: OnceLock<()> = OnceLock::new();

/// Start a forkserver. This function will handle all communication
/// with AFL forkserver end, and use `forkserver_parent` to interact
/// with forked child.
///
/// This function will spawn a child in each round, and in the root process,
/// the loop will never return if everything is OK.
///
/// Before invoking this function, you should initialize [`EDGES_MAP_PTR`],
/// [`INPUT_PTR`] and [`INPUT_LENGTH_PTR`] properly. [`map_shared_memory`] and
/// [`map_input_shared_memory`] can be used, for example.
pub fn start_forkserver<P: ForkserverParent>(
    forkserver_parent: &mut P,
) -> Result<ForkserverState, Error> {
    if FORKSERVER_GUARD.set(()).is_err() {
        return Err(Error::illegal_state("forkserver has been started before"));
    }
    start_forkserver_internal(forkserver_parent)
}

const VERSION: u32 = 0x41464c00 + FS_NEW_VERSION_MAX;
fn start_forkserver_internal<P: ForkserverParent>(
    forkserver_parent: &mut P,
) -> Result<ForkserverState, Error> {
    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    let autotokens_on = unsafe { !__token_start.is_null() && !__token_stop.is_null() };
    let sharedmem_fuzzing = unsafe { SHM_FUZZING == 1 };

    // Parent supports testcases via shared map - and the user wants to use it. Tell AFL.
    // Phone home and tell the parent that we're OK. If parent isn't there, assume we're
    // not running in forkserver mode and just execute program.
    if write_u32_to_forkserver(VERSION).is_err() {
        return Ok(ForkserverState::NoAfl);
    }

    let reply = read_u32_from_forkserver()?;
    if reply != VERSION ^ 0xFFFFFFFF {
        return Err(Error::illegal_state(
            "wrong forkserver message from AFL++ tool",
        ));
    }

    let mut status = FS_NEW_OPT_MAPSIZE;
    if sharedmem_fuzzing {
        status |= FS_NEW_OPT_SHDMEM_FUZZ;
    }
    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    if autotokens_on {
        status |= FS_NEW_OPT_AUTODTCT;
    }
    #[expect(clippy::cast_sign_loss)]
    write_u32_to_forkserver(status as u32)?;

    // Now send the parameters for the set options, increasing by option number

    // FS_NEW_OPT_MAPSIZE - we always send the map size
    write_u32_to_forkserver(unsafe { __afl_map_size as u32 })?;

    // FS_NEW_OPT_AUTODICT - send autotokens
    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    if autotokens_on {
        #[expect(clippy::cast_sign_loss)]
        let tokens_len = unsafe { __token_stop.offset_from(__token_start) } as u32;
        write_u32_to_forkserver(tokens_len).inspect_err(|_| {
            log::error!("Error: could not send autotokens len");
        })?;
        write_all_to_forkserver(unsafe {
            core::slice::from_raw_parts(__token_start, tokens_len as usize)
        })
        .inspect_err(|_| {
            log::error!("could not send autotokens");
        })?;
    }

    // send welcome message as final message
    write_u32_to_forkserver(VERSION)?;

    forkserver_parent.pre_fuzzing()?;

    loop {
        // Wait for parent by reading from the pipe. Abort if read fails.

        let was_killed = read_u32_from_forkserver()?;

        let fork_result = forkserver_parent.spawn_child(was_killed != 0)?;

        match fork_result {
            ForkResult::Child => {
                // FORKSRV_FD is for communication with AFL, we don't need it in the child
                let _ = nix::unistd::close(FORKSRV_R_FD.as_raw_fd());
                let _ = nix::unistd::close(FORKSRV_W_FD.as_raw_fd());
                return Ok(ForkserverState::Child);
            }
            ForkResult::Parent(child_pid) => {
                #[expect(clippy::cast_sign_loss)]
                write_u32_to_forkserver(child_pid.pid as u32).inspect_err(|_| {
                    log::error!("write to afl-fuzz");
                })?;
            }
        }

        let status = forkserver_parent.handle_child_requests()?;

        // Relay wait status to AFL pipe, then loop back.
        #[expect(clippy::cast_sign_loss)]
        write_u32_to_forkserver(status as u32).inspect_err(|_| {
            log::error!("writing to afl-fuzz");
        })?;
    }
}
