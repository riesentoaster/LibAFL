use std::{
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use libafl::{
    corpus::{InMemoryCorpus, OnDiskCorpus},
    events::SimpleEventManager,
    executors::command::CommandConfigurator,
    feedbacks::{ConstFeedback, MaxMapFeedback},
    generators::RandBytesGenerator,
    inputs::{HasTargetBytes, Input},
    monitors::SimplePrintingMonitor,
    mutators::{havoc_mutations, StdScheduledMutator},
    observers::StdMapObserver,
    schedulers::QueueScheduler,
    stages::StdMutationalStage,
    state::StdState,
    Error, Fuzzer, StdFuzzer,
};
use libafl_bolts::{current_nanos, rands::StdRand, tuples::tuple_list, AsSliceMut};
use libafl_bolts::{shmem::ShMem, AsSlice};

use crate::shmem::get_shared_memory;

mod shmem;

pub fn main() -> Result<(), Error> {
    let util = "./target/GNU_coreutils/src/base64";

    let mut shmem = get_shared_memory(util)?;
    let shmem_description = shmem.description();
    let shmem_coverage_slice = shmem.as_slice_mut();

    let coverage_observer = unsafe { StdMapObserver::new("coverage", shmem_coverage_slice) };

    let coverage_feedback = MaxMapFeedback::new(&coverage_observer);

    let mut feedback = coverage_feedback;
    let mut objective = ConstFeedback::new(false);

    let observers = tuple_list!(coverage_observer);

    let mut state = StdState::new(
        StdRand::with_seed(current_nanos()),
        InMemoryCorpus::new(),
        OnDiskCorpus::new(PathBuf::from("./crashes")).unwrap(),
        &mut feedback,
        &mut objective,
    )
    .expect("Could not create state");

    let monitor = SimplePrintingMonitor::new();
    let mut mgr = SimpleEventManager::new(monitor);
    let scheduler = QueueScheduler::new();
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    // Create the executor for an in-process function with just one observer
    #[derive(Debug)]
    struct MyExecutor {
        util: String,
        shmem_coverage_description: String,
    }

    impl CommandConfigurator for MyExecutor {
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

    let mut executor = MyExecutor {
        util: String::from(util),
        shmem_coverage_description: serde_json::to_string(&shmem_description)
            .expect("Could not stringify shared memory description"),
    }
    .into_executor(observers);

    let mut generator = RandBytesGenerator::new(8);
    state
        .generate_initial_inputs(&mut fuzzer, &mut executor, &mut generator, &mut mgr, 8)
        .expect("Failed to generate the initial corpus");

    let mut stages = tuple_list!(StdMutationalStage::new(StdScheduledMutator::new(
        havoc_mutations()
    )));

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Error in the fuzzing loop");
    Ok(())
}
