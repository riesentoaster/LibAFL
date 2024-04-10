use core::fmt::Debug;

use libafl_bolts::Named;
use serde::{Deserialize, Serialize};

use libafl::{
    events::EventFirer,
    executors::ExitKind,
    feedbacks::{DefaultFeedbackFactory, Feedback},
    observers::ObserversTuple,
    state::State,
    Error,
};

/// A [`ActualCrashFeedback`] reports as interesting if the target crashed, and ignores non-zero return values.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActualCrashFeedback {}

impl<S> Feedback<S> for ActualCrashFeedback
where
    S: State,
{
    #[allow(clippy::wrong_self_convention)]
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &S::Input,
        _observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        if let ExitKind::Crash = exit_kind {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Named for ActualCrashFeedback {
    #[inline]
    fn name(&self) -> &str {
        "ActualCrashFeedback"
    }
}

impl ActualCrashFeedback {
    /// Creates a new [`ActualCrashFeedback`]
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ActualCrashFeedback {
    fn default() -> Self {
        Self::new()
    }
}

/// A feedback factory for crash feedbacks
pub type ActualCrashFeedbackFactory = DefaultFeedbackFactory<ActualCrashFeedback>;
