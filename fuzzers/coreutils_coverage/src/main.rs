use std::path::PathBuf;

use executor::CoverageCommandExecutor;
use libafl::{
    corpus::{InMemoryCorpus, OnDiskCorpus},
    events::SimpleEventManager,
    executors::command::CommandConfigurator,
    feedbacks::{ConstFeedback, MaxMapFeedback},
    generators::RandBytesGenerator,
    monitors::SimplePrintingMonitor,
    mutators::{havoc_mutations, StdScheduledMutator},
    observers::StdMapObserver,
    schedulers::QueueScheduler,
    stages::StdMutationalStage,
    state::StdState,
    Error, Fuzzer, StdFuzzer,
};
use libafl_bolts::shmem::ShMem;
use libafl_bolts::{current_nanos, rands::StdRand, tuples::tuple_list, AsSliceMut};

mod executor;
mod shmem;
use crate::shmem::get_shared_memory;

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

    let mut executor =
        CoverageCommandExecutor::new(util, &shmem_description).into_executor(observers);

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
