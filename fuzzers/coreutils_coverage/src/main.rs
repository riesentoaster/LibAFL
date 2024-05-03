mod executor;
mod generator;
mod input;
mod mutator;
mod shmem;

use std::path::PathBuf;

use executor::CoverageCommandExecutor;
use generator::Base64Generator;

use mutator::{Base64FlipDecodeMutator, Base64FlipIgnoreGarbageMutator, Base64WrapContentMutator};
use shmem::get_shared_memory;

use libafl::{
    corpus::{InMemoryCorpus, OnDiskCorpus},
    events::{EventConfig, Launcher, LlmpRestartingEventManager},
    feedbacks::{ConstFeedback, MaxMapFeedback},
    monitors::MultiMonitor,
    mutators::{havoc_mutations, StdScheduledMutator},
    observers::StdMapObserver,
    schedulers::QueueScheduler,
    stages::StdMutationalStage,
    state::StdState,
    Error, Fuzzer, StdFuzzer,
};
use libafl_bolts::{
    cli::parse_args,
    core_affinity::CoreId,
    current_nanos,
    rands::StdRand,
    shmem::{ShMemProvider, StdShMemProvider},
    tuples::tuple_list,
    AsSliceMut,
};
use libafl_bolts::{shmem::ShMem, tuples::Append};

pub fn main() -> Result<(), Error> {
    let options = parse_args();

    let util = "./target/GNU_coreutils/src/base64";
    let monitor = MultiMonitor::new(|s| println!("{s}"));

    let run_client = |state: Option<_>,
                      mut mgr: LlmpRestartingEventManager<_, _, _>,
                      core_id: CoreId|
     -> Result<(), Error> {
        let mut shmem = get_shared_memory(util)?;

        let shmem_description = shmem.description();
        let shmem_coverage_slice = shmem.as_slice_mut();

        let coverage_observer = unsafe { StdMapObserver::new("coverage", shmem_coverage_slice) };

        let coverage_feedback = MaxMapFeedback::new(&coverage_observer);

        let mut feedback = coverage_feedback;
        let mut objective = ConstFeedback::new(false);

        let observers = tuple_list!(coverage_observer);

        let mut state = state.unwrap_or_else(|| {
            StdState::new(
                StdRand::with_seed(current_nanos()),
                InMemoryCorpus::new(),
                OnDiskCorpus::new(PathBuf::from(&options.output)).unwrap(),
                &mut feedback,
                &mut objective,
            )
            .unwrap()
        });

        let scheduler = QueueScheduler::new();
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

        let mut executor =
            CoverageCommandExecutor::new(&shmem_description, observers, util, core_id.into());

        let mut generator = Base64Generator::new(8);

        if state.must_load_initial_inputs() {
            state.generate_initial_inputs(
                &mut fuzzer,
                &mut executor,
                &mut generator,
                &mut mgr,
                8,
            )?
        }

        let mut stages = tuple_list!(StdMutationalStage::new(StdScheduledMutator::new(
            havoc_mutations()
                .append(Base64FlipDecodeMutator)
                .append(Base64FlipIgnoreGarbageMutator)
                .append(Base64WrapContentMutator)
        )));

        fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        // .expect("Error in the fuzzing loop")
    };

    let launcher_shmem_provider = StdShMemProvider::new()?;

    Launcher::builder()
        .configuration(EventConfig::AlwaysUnique)
        .shmem_provider(launcher_shmem_provider)
        .monitor(monitor)
        .run_client(run_client)
        .cores(&options.cores)
        .broker_port(options.broker_port)
        .stdout_file(Some(&options.stdout))
        .remote_broker_addr(options.remote_broker_addr)
        .build()
        .launch()
}
