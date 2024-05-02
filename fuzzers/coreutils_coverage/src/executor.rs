use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    io::Write,
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
            .stdin(Stdio::piped())
            .env(
                "LD_PRELOAD",
                "./target/release/libsetup_guard_redirection.so",
            )
            .args(input.get_args())
            .arg(&self.shmem_coverage_description);
        command.stderr(Stdio::null()).stdout(Stdio::null());

        let child = command.spawn().expect("failed to start process");
        child
            .stdin
            .as_ref()
            .expect("failed to get stdin ref")
            .write_all(input.get_stdin())
            .map_err(|e| {
                Error::illegal_state(format!(
                    "Could not write input to stdin with error {:?} for input {:?}",
                    e,
                    serde_json::to_string_pretty(&input).expect("Serialization error")
                ))
            })?;

        Ok(child)
    }

    fn exec_timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
}
