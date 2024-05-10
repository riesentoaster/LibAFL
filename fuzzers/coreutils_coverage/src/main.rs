mod base64;
mod generic;
mod metadata_structs;

mod shmem;

use std::path::{Path, PathBuf};

use base64::{
    Base64FlipDecodeMutator, Base64FlipIgnoreGarbageMutator, Base64Generator, Base64Input,
    Base64WrapContentMutator,
};

use generic::{CoverageCommandExecutor, DiffWithMetadataFeedback, InputLoggerFeedback};
use metadata_structs::{
    vec_string_mapper, InputMetadata, StdErrBinaryDiffMetadata, StdOutDiffMetadata,
};

use libafl::{
    corpus::{InMemoryCorpus, OnDiskCorpus},
    events::{EventConfig, Launcher, LlmpRestartingEventManager},
    executors::DiffExecutor,
    feedback_and, feedback_or, feedback_or_fast,
    feedbacks::{ConstFeedback, CrashFeedback, MaxMapFeedback, TimeFeedback, TimeoutFeedback},
    monitors::MultiMonitor,
    mutators::{havoc_mutations, StdScheduledMutator},
    observers::{MultiMapObserver, StdErrObserver, StdMapObserver, StdOutObserver, TimeObserver},
    schedulers::QueueScheduler,
    stages::StdMutationalStage,
    state::StdState,
    Error, Fuzzer, StdFuzzer,
};

use shmem::{get_guard_num, make_shmem_persist};

use libafl_bolts::{
    cli::parse_args,
    core_affinity::CoreId,
    current_nanos,
    ownedref::OwnedMutSlice,
    rands::StdRand,
    shmem::{MmapShMemProvider, ShMem, ShMemProvider, StdShMemProvider},
    tuples::{tuple_list, Append},
    AsSliceMut,
};

pub static UUTILS_PREFIX: &str = "./target/uutils_coreutils/target/release/";
pub static GNU_PREFIX: &str = "./target/GNU_coreutils/src/";

pub fn main() {
    let util = "base64";
    match fuzz(util) {
        Ok(_) => (),
        Err(Error::ShuttingDown) => {
            println!("Orderly shutdown");
        }
        Err(e) => {
            println!("Error: {:#?}", e);
        }
    }
}

fn fuzz(util: &str) -> Result<(), Error> {
    let uutils_path = format!("{UUTILS_PREFIX}{util}");
    let gnu_path = format!("{GNU_PREFIX}{util}");
    if !Path::new(&uutils_path).exists() {
        return Err(Error::illegal_argument(format!(
            "Util {util} not found in prefix {UUTILS_PREFIX}"
        )));
    }
    if !Path::new(&gnu_path).exists() {
        return Err(Error::illegal_argument(format!(
            "Util {util} not found in prefix {GNU_PREFIX}"
        )));
    }

    let options = parse_args();

    let monitor = MultiMonitor::new(|s| println!("{s}"));

    let uutils_guard_num = get_guard_num(&uutils_path)?;
    let gnu_guard_num = get_guard_num(&gnu_path)?;

    let mut shmem_provider = MmapShMemProvider::default();

    let run_client = |state: Option<_>,
                      mut mgr: LlmpRestartingEventManager<_, _, _>,
                      core_id: CoreId|
     -> Result<(), Error> {
        let mut uutils_coverage_shmem = shmem_provider
            .new_shmem(uutils_guard_num * 4)
            .expect("Could not get the shared memory map");
        let mut gnu_coverage_shmem = shmem_provider
            .new_shmem(gnu_guard_num * 4)
            .expect("Could not get the shared memory map");

        let uutils_coverage_shmem_description = uutils_coverage_shmem.description();
        let gnu_coverage_shmem_description = gnu_coverage_shmem.description();
        make_shmem_persist(&uutils_coverage_shmem_description)?;
        make_shmem_persist(&gnu_coverage_shmem_description)?;

        let uutils_coverage_slice = uutils_coverage_shmem.as_slice_mut();
        let gnu_coverage_slice = gnu_coverage_shmem.as_slice_mut();

        let combined_coverage = unsafe {
            vec![
                OwnedMutSlice::from_raw_parts_mut(
                    uutils_coverage_slice.as_mut_ptr(),
                    uutils_coverage_slice.len(),
                ),
                OwnedMutSlice::from_raw_parts_mut(
                    gnu_coverage_slice.as_mut_ptr(),
                    gnu_coverage_slice.len(),
                ),
            ]
        };

        let uutils_coverage_observer =
            unsafe { StdMapObserver::new("uutils-coverage", uutils_coverage_slice) };
        let gnu_coverage_observer =
            unsafe { StdMapObserver::new("gnu-coverage", gnu_coverage_slice) };

        let combined_coverage_observer =
            MultiMapObserver::differential("combined-coverage", combined_coverage);

        let uutils_stdout_observer = StdOutObserver::new("uutils-stdout-observer");
        let uutils_stderr_observer = StdErrObserver::new("uutils-stderr-observer");
        let gnu_stdout_observer = StdOutObserver::new("gnu-stdout-observer");
        let gnu_stderr_observer = StdErrObserver::new("gnu-stderr-observer");

        let uutils_time_observer = TimeObserver::new("uutils-time-observer");
        let gnu_time_observer = TimeObserver::new("gnu-time-observer");

        let stdout_diff_feedback = DiffWithMetadataFeedback::new(
            "stdout-diff-feedback",
            &uutils_stdout_observer,
            &gnu_stdout_observer,
            |o| {
                vec_string_mapper(&o.stdout)
                    .replace(&uutils_path, "[libafl: util_path]")
                    .to_owned()
            },
            |o| {
                vec_string_mapper(&o.stdout)
                    .replace(&gnu_path, "[libafl: util_path]")
                    .to_owned()
            },
            |o1, o2| {
                StdOutDiffMetadata::new(
                    vec_string_mapper(&o1.stdout)
                        .replace(&uutils_path, "[libafl: util_path]")
                        .to_owned(),
                    vec_string_mapper(&o2.stdout)
                        .replace(&gnu_path, "[libafl: util_path]")
                        .to_owned(),
                )
            },
        )?;
        let stderr_diff_feedback = DiffWithMetadataFeedback::new(
            "stderr-diff-feedback",
            &uutils_stderr_observer,
            &gnu_stderr_observer,
            |o| o.stderr.as_ref().map_or(false, |e| !e.is_empty()),
            |o| o.stderr.as_ref().map_or(false, |e| !e.is_empty()),
            |o1, o2| {
                StdErrBinaryDiffMetadata::new(
                    vec_string_mapper(&o1.stderr).to_owned(),
                    vec_string_mapper(&o2.stderr).to_owned(),
                )
            },
        )?;

        let mut feedback = feedback_or!(
            MaxMapFeedback::new(&combined_coverage_observer) // pseudo_feedback
        );

        let actual_objective = feedback_or_fast!(
            stdout_diff_feedback,
            stderr_diff_feedback,
            CrashFeedback::new(),
            TimeoutFeedback::new()
        );
        let pseudo_objective = feedback_or!(
            InputLoggerFeedback::new("input-logger-feedback", |i: &Base64Input| {
                InputMetadata::new(format!("input-logger-feedback: {}", i))
            }),
            TimeFeedback::new(&uutils_time_observer),
            TimeFeedback::new(&gnu_time_observer),
            ConstFeedback::new(true) // to ensure the whole block to be interesting
        );

        let mut objective = feedback_and!(actual_objective, pseudo_objective);

        let uutils_observers = tuple_list!(
            uutils_coverage_observer,
            uutils_stdout_observer,
            uutils_stderr_observer,
            uutils_time_observer
        );

        let gnu_observers = tuple_list!(
            gnu_coverage_observer,
            gnu_stdout_observer,
            gnu_stderr_observer,
            gnu_time_observer
        );

        let combined_observers = tuple_list!(combined_coverage_observer);

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

        let uutils_executor = CoverageCommandExecutor::new(
            &uutils_coverage_shmem_description,
            uutils_observers,
            &uutils_path,
            format!("uutils-{:?}", core_id.0),
        );

        let gnu_executor = CoverageCommandExecutor::new(
            &gnu_coverage_shmem_description,
            gnu_observers,
            &gnu_path,
            format!("gnu-{:?}", core_id.0),
        );

        let mut diff_executor =
            DiffExecutor::new(uutils_executor, gnu_executor, combined_observers);

        if state.must_load_initial_inputs() {
            state.generate_initial_inputs(
                &mut fuzzer,
                &mut diff_executor,
                &mut Base64Generator::new(8),
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

        fuzzer.fuzz_loop(&mut stages, &mut diff_executor, &mut state, &mut mgr)
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
