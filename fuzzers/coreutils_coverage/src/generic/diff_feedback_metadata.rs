use libafl::{
    corpus::Testcase,
    events::EventFirer,
    executors::ExitKind,
    feedbacks::Feedback,
    inputs::Input,
    observers::{Observer, ObserversTuple},
    state::State,
    Error, HasMetadata,
};
use libafl_bolts::{
    serdeany::SerdeAny,
    tuples::{MatchNameRef, Reference, Referenceable},
    Named,
};

use std::{borrow::Cow, marker::PhantomData};

/// A [`DiffWithMetadataFeedback`] behaves the same as a [`DiffFeedback`], except that it will also add the compared values as metadata in case of an interesting input.
pub struct DiffWithMetadataFeedback<O1, O2, FE1, FE2, RE, FM, RM, I, S>
where
    FE1: FnMut(&O1) -> RE,
    FE2: FnMut(&O2) -> RE,
    FM: FnMut(&O1, &O2) -> RM,
    I: ToString,
{
    name: Cow<'static, str>,
    o1: Reference<O1>,
    o2: Reference<O2>,
    o1_extractor: FE1,
    o2_extractor: FE2,
    mapper: FM,
    phantom: PhantomData<(I, S)>,
    is_interesting: bool,
}

impl<O1, O2, FE1, FE2, RE, FM, RM, I, S>
    DiffWithMetadataFeedback<O1, O2, FE1, FE2, RE, FM, RM, I, S>
where
    FE1: FnMut(&O1) -> RE,
    FE2: FnMut(&O2) -> RE,
    FM: FnMut(&O1, &O2) -> RM,
    O1: Named,
    O2: Named,
    I: ToString,
{
    /// Create a new [`DiffWithMetadataFeedback`] using two observers and a test function.
    pub fn new(
        name: &'static str,
        o1: &O1,
        o2: &O2,
        o1_extractor: FE1,
        o2_extractor: FE2,
        mapper: FM,
    ) -> Result<Self, Error> {
        let o1_ref = o1.reference();
        let o2_ref = o2.reference();
        if o1_ref.name() == o2_ref.name() {
            Err(Error::illegal_argument(format!(
                "DiffFeedback: observer names must be different (both were {})",
                o1_ref.name()
            )))
        } else {
            Ok(Self {
                o1: o1_ref,
                o2: o2_ref,
                name: Cow::from(name),
                o1_extractor,
                o2_extractor,
                mapper,
                is_interesting: false,
                phantom: PhantomData,
            })
        }
    }
}

impl<O1, O2, FE1, FE2, RE, FM, RM, I, S> Feedback<S>
    for DiffWithMetadataFeedback<O1, O2, FE1, FE2, RE, FM, RM, I, S>
where
    FE1: FnMut(&O1) -> RE,
    FE2: FnMut(&O2) -> RE,
    RE: Eq,
    FM: FnMut(&O1, &O2) -> RM,

    RM: SerdeAny,
    I: Input + ToString,
    S: HasMetadata + State<Input = I>,
    O1: Observer<S>,
    O2: Observer<S>,
{
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &<S>::Input,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        fn err(name: &str) -> Error {
            Error::illegal_argument(format!("DiffFeedback: observer {name} not found"))
        }
        let o1: &O1 = observers.get(&self.o1).ok_or_else(|| err(self.o1.name()))?;
        let o2: &O2 = observers.get(&self.o2).ok_or_else(|| err(self.o2.name()))?;
        let is_interesting = (self.o1_extractor)(o1) != (self.o2_extractor)(o2);
        self.is_interesting = is_interesting;
        Ok(is_interesting)
    }

    fn append_metadata<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        observers: &OT,
        testcase: &mut Testcase<<S>::Input>,
    ) -> Result<(), Error>
    where
        OT: ObserversTuple<S>,
        EM: EventFirer<State = S>,
    {
        if !self.is_interesting {
            return Ok(());
        }

        let err =
            |name| Error::illegal_argument(format!("DiffFeedback: observer {name} not found"));
        let o1: &O1 = observers.get(&self.o1).ok_or_else(|| err(self.o1.name()))?;
        let o2: &O2 = observers.get(&self.o2).ok_or_else(|| err(self.o2.name()))?;

        testcase.add_metadata((self.mapper)(o1, o2));
        Ok(())
    }
}

impl<O1, O2, FE1, FE2, RE, FM, RM, I, S> Named
    for DiffWithMetadataFeedback<O1, O2, FE1, FE2, RE, FM, RM, I, S>
where
    FE1: FnMut(&O1) -> RE,
    FE2: FnMut(&O2) -> RE,
    FM: FnMut(&O1, &O2) -> RM,
    I: ToString,
{
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}
