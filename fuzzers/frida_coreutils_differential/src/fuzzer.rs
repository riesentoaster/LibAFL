//! A libfuzzer-like fuzzer with llmp-multithreading support and restarts
//! The example harness is built for libpng.
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::{fs::OpenOptions, os::fd::AsRawFd, path::PathBuf};

use frida_gum::Gum;
use libafl::{
    corpus::{CachedOnDiskCorpus, OnDiskCorpus},
    events::{launcher::Launcher, llmp::LlmpRestartingEventManager, EventConfig},
    executors::{inprocess::InProcessExecutor, ExitKind, InProcessForkExecutor},
    feedback_or, feedback_or_fast,
    feedbacks::{MaxMapFeedback, TimeFeedback, TimeoutFeedback},
    fuzzer::{Fuzzer, StdFuzzer},
    generators::RandPrintablesGenerator,
    inputs::{BytesInput, HasTargetBytes},
    monitors::MultiMonitor,
    mutators::scheduled::{havoc_mutations, StdScheduledMutator},
    observers::{HitcountsMapObserver, StdMapObserver, TimeObserver},
    schedulers::{IndexesLenTimeMinimizerScheduler, QueueScheduler},
    stages::StdMutationalStage,
    state::StdState,
    Error,
};
use libafl_bolts::{
    cli::{parse_args, FuzzerOptions},
    current_nanos,
    rands::StdRand,
    shmem::{ShMemProvider, StdShMemProvider},
    tuples::tuple_list,
    AsSlice,
};

use libafl_frida::{
    coverage_rt::{CoverageRuntime, MAP_SIZE},
    executor::FridaInProcessExecutor,
    helper::FridaInstrumentationHelper,
};

pub unsafe fn lib(main: extern "C" fn(i32, *const *const u8, *const *const u8) -> i32) {
    color_backtrace::install();

    let options = parse_args();

    let frida_harness = |input: &BytesInput| {
        let target = input.target_bytes();
        let buf = target.as_slice();
        let len = buf.len().to_string();
        let binary_path = options.harness.clone().unwrap();
        let binary_name = binary_path.to_str().unwrap();

        let argv: [*const u8; 3] = [
            binary_name.as_ptr().cast(),
            len.as_ptr().cast(),
            buf.as_ptr().cast(),
        ];

        let env: [*const u8; 0] = [];

        eprintln!("{}", main(3, argv.as_ptr(), env.as_ptr()));
        ExitKind::Ok
    };

    unsafe {
        match fuzz(&options, &frida_harness) {
            Ok(()) => println!("\nFinished fuzzing with return value. Good bye."),
            Err(Error::ShuttingDown) => {
                println!("\nFinished fuzzing with orderly shutdown. Good bye.")
            }
            Err(e) => panic!("Error during fuzzing: {e:?}"),
        }
    }
}

/// The actual fuzzer
unsafe fn fuzz(
    options: &FuzzerOptions,
    mut frida_harness: &dyn Fn(&BytesInput) -> ExitKind,
) -> Result<(), Error> {
    // 'While the stats are state, they are usually used in the broker - which is likely never restarted
    let monitor = MultiMonitor::new(|s| println!("{s}"));
    let shmem_provider = StdShMemProvider::new()?;

    let mut run_client =
        |state: Option<_>, mut mgr: LlmpRestartingEventManager<_, _, _>, _core_id| {
            let stdout = OpenOptions::new()
                .append(true)
                .create(true)
                .open("stdout.txt")
                .expect("Failed to open output file");
            let stderr = OpenOptions::new()
                .append(true)
                .create(true)
                .open("stderr.txt")
                .expect("Failed to open output file");
            libc::dup2(stdout.as_raw_fd(), libc::STDOUT_FILENO);
            libc::dup2(stderr.as_raw_fd(), libc::STDERR_FILENO);

            let gum = Gum::obtain();
            let mut frida_helper =
                FridaInstrumentationHelper::new(&gum, options, tuple_list!(CoverageRuntime::new()));

            // Create an observation channel using the coverage map
            let edges_observer = HitcountsMapObserver::new(StdMapObserver::from_mut_ptr(
                "edges",
                frida_helper.map_mut_ptr().unwrap(),
                MAP_SIZE,
            ));

            // Create an observation channel to keep track of the execution time
            let time_observer = TimeObserver::new("time");

            // Feedback to rate the interestingness of an input
            // This one is composed by two Feedbacks in OR
            let mut feedback = feedback_or!(
                // New maximization map feedback linked to the edges observer and the feedback state
                MaxMapFeedback::tracking(&edges_observer, true, false),
                // Time feedback, this one does not need a feedback state
                TimeFeedback::with_observer(&time_observer)
            );

            let mut objective = feedback_or_fast!(TimeoutFeedback::new());

            // If not restarting, create a State from scratch
            let mut state = state.unwrap_or_else(|| {
                StdState::new(
                    // RNG
                    StdRand::with_seed(current_nanos()),
                    // Corpus that will be evolved, we keep it in memory for performance
                    CachedOnDiskCorpus::no_meta(PathBuf::from("./corpus_discovered"), 64).unwrap(),
                    // Corpus in which we store solutions (crashes in this example),
                    // on disk so the user can get them after stopping the fuzzer
                    OnDiskCorpus::new(options.output.clone()).unwrap(),
                    &mut feedback,
                    &mut objective,
                )
                .unwrap()
            });

            println!("We're a client, let's fuzz :)");

            // Setup a basic mutator with a mutational stage
            let mutator = StdScheduledMutator::new(havoc_mutations());

            // A minimization+queue policy to get testcasess from the corpus
            let scheduler = IndexesLenTimeMinimizerScheduler::new(QueueScheduler::new());

            // A fuzzer with feedbacks and a corpus scheduler
            let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

            let observers = tuple_list!(edges_observer, time_observer);

            // Create the executor for an in-process function with just one observer for edge coverage
            let mut executor = FridaInProcessExecutor::new(
                &gum,
                InProcessForkExecutor::new(
                    &mut frida_harness,
                    observers,
                    &mut fuzzer,
                    &mut state,
                    &mut mgr,
                    10,
                    shmem_provider,
                )?,
                &mut frida_helper,
            );

            //         harness_fn: &'a mut H,
            // observers: OT,
            // fuzzer: &mut Z,
            // state: &mut S,
            // event_mgr: &mut EM,
            // timeout: Duration,
            // shmem_provider: SP,

            // In case the corpus is empty (on first run), reset
            if state.must_load_initial_inputs() {
                state
                    .generate_initial_inputs(
                        &mut fuzzer,
                        &mut executor,
                        &mut RandPrintablesGenerator::new(32),
                        &mut mgr,
                        8,
                    )
                    .expect("Failed to generate the initial corpus");
            }

            let mut stages = tuple_list!(StdMutationalStage::new(mutator));

            fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)?;

            Ok(())
        };

    Launcher::builder()
        .configuration(EventConfig::AlwaysUnique)
        .shmem_provider(shmem_provider)
        .monitor(monitor)
        .run_client(&mut run_client)
        .cores(&options.cores)
        .broker_port(options.broker_port)
        .stdout_file(Some(&options.stdout))
        .remote_broker_addr(options.remote_broker_addr)
        .build()
        .launch()
}
