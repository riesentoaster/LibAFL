use std::{
    io::Write,
    process::{Child, Command, Stdio},
    time::Duration,
};

use libafl::{
    executors::command::CommandConfigurator,
    inputs::{HasTargetBytes, Input},
    Error,
};
use libafl_bolts::{shmem::ShMemDescription, AsSlice};

// Create the executor for an in-process function with just one observer
#[derive(Debug)]
pub struct CoverageCommandExecutor {
    util: String,
    shmem_coverage_description: String,
}

impl CoverageCommandExecutor {
    pub fn new(util: &str, shmem_coverage_description: &ShMemDescription) -> Self {
        let serialized_description = serde_json::to_string(&shmem_coverage_description)
            .expect("Could not stringify shared memory description");
        Self {
            util: String::from(util),
            shmem_coverage_description: serialized_description,
        }
    }
}

impl CommandConfigurator for CoverageCommandExecutor {
    fn spawn_child<I: Input + HasTargetBytes>(&mut self, input: &I) -> Result<Child, Error> {
        let mut command = Command::new(&self.util);

        command
            .stdin(Stdio::piped())
            .env(
                "LD_PRELOAD",
                "./target/release/libsetup_guard_redirection.so",
            )
            .arg(&self.shmem_coverage_description);

        let child = command.spawn().expect("failed to start process");

        child
            .stdin
            .as_ref()
            .expect("failed to get stdin ref")
            .write_all(input.target_bytes().as_slice())
            .map_err(|e| {
                Error::illegal_state(format!(
                    "Could not write input to stdin with error {:?} for input {:?}",
                    e,
                    serde_json::to_string(&input).expect("Serialization error")
                ))
            })?;

        Ok(child)
    }

    fn exec_timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
}
