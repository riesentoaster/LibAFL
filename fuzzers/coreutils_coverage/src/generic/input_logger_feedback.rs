use std::borrow::Cow;

use libafl::{
    corpus::Testcase, events::EventFirer, executors::ExitKind, feedbacks::Feedback,
    observers::ObserversTuple, state::State, Error, HasMetadata, HasNamedMetadata,
};
use libafl_bolts::{serdeany::SerdeAny, Named};

pub struct InputLoggerFeedback<I, R, F>
where
    F: FnMut(&I) -> R,
{
    input: Option<I>,
    name: Cow<'static, str>,
    mapper: F,
}

impl<I, R, F> InputLoggerFeedback<I, R, F>
where
    F: FnMut(&I) -> R,
{
    pub fn new(name: &'static str, mapper: F) -> Self {
        Self {
            input: None,
            name: Cow::Borrowed(name),
            mapper,
        }
    }
}

impl<S, R, F> Feedback<S> for InputLoggerFeedback<S::Input, R, F>
where
    F: FnMut(&S::Input) -> R,
    R: SerdeAny,
    S: State + HasNamedMetadata,
    S::Input: SerdeAny + Clone,
{
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        input: &<S>::Input,
        _observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        self.input = Some(input.clone());
        Ok(false)
    }

    fn append_metadata<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _observers: &OT,
        testcase: &mut Testcase<<S>::Input>,
    ) -> Result<(), Error>
    where
        OT: ObserversTuple<S>,
        EM: EventFirer<State = S>,
    {
        match &self.input {
            None => Err(Error::illegal_state(
                "Should have stored input at this point",
            )),
            Some(input) => {
                testcase.add_metadata((self.mapper)(input));
                Ok(())
            }
        }
    }
}

impl<I, R, F> Named for InputLoggerFeedback<I, R, F>
where
    F: FnMut(&I) -> R,
{
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}
