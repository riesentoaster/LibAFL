use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    fs::File,
    io::{Seek, SeekFrom, Write},
    marker::PhantomData,
    process::{Child, Command, Stdio},
    time::Duration,
};

use libafl::{
    executors::{command::CommandConfigurator, CommandExecutor},
    state::State,
    Error,
};
use libafl_bolts::{shmem::ShMemDescription, tuples::MatchName};
use serde::Serialize;

// Create the executor for an in-process function with just one observer
#[derive(Debug)]
pub struct CoverageCommandExecutor<I: ExtractsToCommand> {
    shmem_coverage_description: String,
    phantom: PhantomData<I>,
}
impl<I: ExtractsToCommand> CoverageCommandExecutor<I> {
    pub fn new<OT, S>(
        shmem_coverage_description: &ShMemDescription,
        observers: OT,
    ) -> CommandExecutor<OT, S, CoverageCommandExecutor<I>>
    where
        S: State,
        S::Input: ExtractsToCommand,
        OT: MatchName,
    {
        let serialized_description = serde_json::to_string(&shmem_coverage_description)
            .expect("Could not stringify shared memory description");

        let configurator = Self {
            shmem_coverage_description: serialized_description,
            phantom: PhantomData,
        };
        configurator.into_executor(observers)
    }
}

pub trait ExtractsToCommand: Serialize {
    fn get_program(&self) -> &OsString;
    fn get_stdin(&self) -> &Vec<u8>;
    fn get_args<'a>(&self) -> Vec<Cow<'a, OsStr>>;
}

impl<I> CommandConfigurator<I> for CoverageCommandExecutor<I>
where
    I: ExtractsToCommand,
{
    fn spawn_child(&mut self, input: &I) -> Result<Child, Error> {
        let mut command = Command::new(input.get_program());
        command
            .env(
                "LD_PRELOAD",
                "./target/release/libsetup_guard_redirection.so",
            )
            .args(input.get_args())
            .arg(&self.shmem_coverage_description)
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .stdin(pseudo_pipe(input.get_stdin())?);

        let child = command.spawn().expect("failed to start process");
        Ok(child)
    }

    fn exec_timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
}

/// Creates a [`Stdio`] object that can be used to write data to a [`Command`]'s `stdin`.
///
/// The implementation relies on an in-memory temp file written to `/dev/shm/`.
///
/// # Errors on
///
/// This function will return an error if the underlying os functions error.
fn pseudo_pipe(data: &[u8]) -> Result<Stdio, Error> {
    let mut temp_file = File::create("/dev/shm/temp")
        .map_err(|e| Error::os_error(e, "Could not create temp file"))?;
    temp_file
        .write_all(data)
        .map_err(|e| Error::os_error(e, "Could not write data to temp file"))?;
    // temp_file.sync_all().expect("Could not wait until data is done writing");
    temp_file
        .seek(SeekFrom::Start(0))
        .map_err(|e| Error::os_error(e, "Could reset seek in temp file"))?;
    Ok(Stdio::from(temp_file))
}
