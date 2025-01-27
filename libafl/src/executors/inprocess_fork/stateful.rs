//! A version of `InProcessForkExecutor` with a state accessible from the harness.
//!
//! The `StatefulGenericInProcessForkExecutor` to do forking before executing the harness in-process.
//! The harness can access internal state.
use core::{
    fmt::{self, Debug, Formatter},
    time::Duration,
};

use libafl_bolts::{
    shmem::ShMemProvider,
    tuples::{tuple_list, RefIndexable},
};
use nix::unistd::{fork, ForkResult};

use crate::{
    executors::{
        hooks::ExecutorHooksTuple, inprocess_fork::GenericInProcessForkExecutorInner, Executor,
        ExitKind, HasObservers,
    },
    observers::ObserversTuple,
    state::HasExecutions,
    Error,
};

/// The `StatefulInProcessForkExecutor` with no user hooks
pub type StatefulInProcessForkExecutor<'a, ES, H, I, OT, S, SP> =
    StatefulGenericInProcessForkExecutor<'a, ES, H, (), I, OT, S, SP>;

impl<'a, H, I, OT, S, SP, ES> StatefulInProcessForkExecutor<'a, ES, H, I, OT, S, SP>
where
    OT: ObserversTuple<I, S>,
    SP: ShMemProvider,
{
    /// The constructor for `InProcessForkExecutor`
    pub fn new(
        harness_fn: &'a mut H,
        exposed_executor_state: ES,
        observers: OT,
        state: &mut S,
        timeout: Duration,
        shmem_provider: SP,
    ) -> Result<Self, Error> {
        Self::with_hooks(
            tuple_list!(),
            harness_fn,
            exposed_executor_state,
            observers,
            state,
            timeout,
            shmem_provider,
        )
    }
}

/// [`StatefulGenericInProcessForkExecutor`] is an executor that forks the current process before each execution. Harness can access some internal state.
pub struct StatefulGenericInProcessForkExecutor<'a, ES, H, HT, I, OT, S, SP> {
    /// The harness function, being executed for each fuzzing loop execution
    harness_fn: &'a mut H,
    /// The state used as argument of the harness
    pub exposed_executor_state: ES,
    /// Inner state of the executor
    pub inner: GenericInProcessForkExecutorInner<HT, I, OT, S, SP>,
}

impl<H, HT, I, OT, S, SP, ES> Debug
    for StatefulGenericInProcessForkExecutor<'_, ES, H, HT, I, OT, S, SP>
where
    HT: Debug,
    OT: Debug,
    SP: Debug,
{
    #[cfg(target_os = "linux")]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenericInProcessForkExecutor")
            .field("GenericInProcessForkExecutionInner", &self.inner)
            .finish()
    }

    #[cfg(not(target_os = "linux"))]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        #[cfg(not(target_os = "linux"))]
        return f
            .debug_struct("GenericInProcessForkExecutor")
            .field("GenericInProcessForkExecutionInner", &self.inner)
            .finish();
    }
}

impl<H, HT, I, OT, S, SP, ES> Executor<I, S>
    for StatefulGenericInProcessForkExecutor<'_, ES, H, HT, I, OT, S, SP>
where
    H: FnMut(&mut ES, &I) -> ExitKind + Sized,
    HT: ExecutorHooksTuple<I, S>,
    S: HasExecutions,
    SP: ShMemProvider,
    OT: ObserversTuple<I, S>,
{
    #[inline]
    fn run_target(&mut self, state: &mut S, input: &I) -> Result<ExitKind, Error> {
        *state.executions_mut() += 1;

        unsafe {
            self.inner.shmem_provider.pre_fork()?;
            match fork() {
                Ok(ForkResult::Child) => {
                    // Child
                    self.inner.pre_run_target_child(state, input)?;
                    (self.harness_fn)(&mut self.exposed_executor_state, input);
                    self.inner.post_run_target_child(state, input);
                    Ok(ExitKind::Ok)
                }
                Ok(ForkResult::Parent { child }) => {
                    // Parent
                    self.inner.parent(child)
                }
                Err(e) => Err(Error::from(e)),
            }
        }
    }
}

impl<'a, H, HT, I, OT, S, SP, ES> StatefulGenericInProcessForkExecutor<'a, ES, H, HT, I, OT, S, SP>
where
    HT: ExecutorHooksTuple<I, S>,
    OT: ObserversTuple<I, S>,
{
    /// Creates a new [`StatefulGenericInProcessForkExecutor`] with custom hooks
    pub fn with_hooks(
        userhooks: HT,
        harness_fn: &'a mut H,
        exposed_executor_state: ES,
        observers: OT,
        state: &mut S,
        timeout: Duration,
        shmem_provider: SP,
    ) -> Result<Self, Error> {
        Ok(Self {
            harness_fn,
            exposed_executor_state,
            inner: GenericInProcessForkExecutorInner::with_hooks(
                userhooks,
                observers,
                state,
                timeout,
                shmem_provider,
            )?,
        })
    }

    /// Retrieve the harness function.
    #[inline]
    pub fn harness(&self) -> &H {
        self.harness_fn
    }

    /// Retrieve the harness function for a mutable reference.
    #[inline]
    pub fn harness_mut(&mut self) -> &mut H {
        self.harness_fn
    }
}

impl<H, HT, I, OT, S, SP, ES> HasObservers
    for StatefulGenericInProcessForkExecutor<'_, ES, H, HT, I, OT, S, SP>
{
    type Observers = OT;

    #[inline]
    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        self.inner.observers()
    }

    #[inline]
    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        self.inner.observers_mut()
    }
}
