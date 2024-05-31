use std::borrow::Cow;

use libafl::{
    inputs::{MutVecInput, UsesInput},
    mutators::{MutationResult, Mutator},
    Error,
};
use libafl_bolts::Named;

pub struct MappingMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    extractor: for<'a> fn(&'a mut S::Input) -> &'a mut Vec<u8>,
    inner: M,
}

impl<S, M> MappingMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    pub fn new(extractor: for<'a> fn(&'a mut S::Input) -> &'a mut Vec<u8>, inner: M) -> Self {
        Self { extractor, inner }
    }
}

impl<S, M> Mutator<S::Input, S> for MappingMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    fn mutate(&mut self, state: &mut S, input: &mut S::Input) -> Result<MutationResult, Error> {
        let mut mut_vec_input: MutVecInput = (self.extractor)(input).into();
        self.inner.mutate(state, &mut mut_vec_input)
    }
}

impl<S, M> Named for MappingMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("MappingMutator")
    }
}
pub struct MappingOptionMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    extractor: for<'a> fn(&'a mut S::Input) -> &'a mut Option<Vec<u8>>,
    inner: M,
}

impl<S, M> MappingOptionMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    pub fn new(
        extractor: for<'a> fn(&'a mut S::Input) -> &'a mut Option<Vec<u8>>,
        inner: M,
    ) -> Self {
        Self { extractor, inner }
    }
}

impl<S, M> Mutator<S::Input, S> for MappingOptionMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    fn mutate(&mut self, state: &mut S, input: &mut S::Input) -> Result<MutationResult, Error> {
        if let Some(extracted) = (self.extractor)(input) {
            let mut mut_vec_input: MutVecInput = extracted.into();
            self.inner.mutate(state, &mut mut_vec_input)
        } else {
            Ok(MutationResult::Skipped)
        }
    }
}

impl<S, M> Named for MappingOptionMutator<S, M>
where
    S: UsesInput,
    for<'a> M: Mutator<MutVecInput<'a>, S>,
{
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("MappingOptionMutator")
    }
}
