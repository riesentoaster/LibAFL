use std::{borrow::Cow, fs::OpenOptions, io::Write};

use libafl::{
    events::EventFirer,
    executors::ExitKind,
    feedbacks::Feedback,
    observers::{Observer, ObserversTuple},
    state::State,
    Error,
};
use libafl_bolts::Named;

pub struct PseudoPrintFeedback<'a, O> {
    log_file: &'a String,
    observer: &'a O,
    extractor: Box<dyn Fn(&O) -> String>,
}

impl<'a, O> PseudoPrintFeedback<'a, O> {
    pub fn new(
        log_file: &'a String,
        observer: &'a O,
        extractor: Box<dyn Fn(&O) -> String>,
    ) -> Self {
        Self {
            log_file,
            observer,
            extractor,
        }
    }
}

impl<'a, O> Named for PseudoPrintFeedback<'a, O> {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("PseudoPrintFeedback")
    }
}

impl<'a, O, S> Feedback<S> for PseudoPrintFeedback<'a, O>
where
    O: Observer<S>,
    S: State,
{
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &<S>::Input,
        _observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.log_file)
            .map_err(|e| Error::os_error(e, "Could not open logfile"))?
            .write_all((*self.extractor)(self.observer).as_bytes())
            .map_err(|e| Error::os_error(e, "Could not write to logfile"))?;
        Ok(false)
    }
}
