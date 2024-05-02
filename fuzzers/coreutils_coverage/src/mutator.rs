use std::borrow::Cow;

use libafl::{
    mutators::{havoc_mutations, MutationResult, Mutator, MutatorsTuple},
    state::{HasCorpus, HasMaxSize, HasRand},
    Error,
};
use libafl_bolts::{rands::Rand, HasLen, Named};

use crate::input::Base64Input;

pub struct Base64FlipDecodeMutator;
impl<S> Mutator<Base64Input, S> for Base64FlipDecodeMutator
where
    S: HasRand,
{
    fn mutate(&mut self, _state: &mut S, input: &mut Base64Input) -> Result<MutationResult, Error> {
        input.decode = !input.decode;
        Ok(MutationResult::Mutated)
    }
}

impl Named for Base64FlipDecodeMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("Base64FlipDecodeMutator");
        &NAME
    }
}
pub struct Base64FlipIgnoreGarbageMutator;
impl<S> Mutator<Base64Input, S> for Base64FlipIgnoreGarbageMutator
where
    S: HasRand,
{
    fn mutate(&mut self, _state: &mut S, input: &mut Base64Input) -> Result<MutationResult, Error> {
        input.ignore_garbage = !input.ignore_garbage;
        Ok(MutationResult::Mutated)
    }
}

impl Named for Base64FlipIgnoreGarbageMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("Base64FlipIgnoreGarbageMutator");
        &NAME
    }
}

pub struct Base64WrapContentMutator;
impl<S> Mutator<Base64Input, S> for Base64WrapContentMutator
where
    S: HasRand,
{
    fn mutate(&mut self, state: &mut S, input: &mut Base64Input) -> Result<MutationResult, Error> {
        match input.wrap {
            Some(_e) => {
                input.wrap = Some(state.rand_mut().next() as i16);
                Ok(MutationResult::Mutated)
            }
            None => Ok(MutationResult::Skipped),
        }
    }
}

impl Named for Base64WrapContentMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("Base64WrapContentMutator");
        &NAME
    }
}
pub struct Base64FlipWrapMutator;
impl<S> Mutator<Base64Input, S> for Base64FlipWrapMutator
where
    S: HasRand,
{
    fn mutate(&mut self, state: &mut S, input: &mut Base64Input) -> Result<MutationResult, Error> {
        match input.wrap {
            None => {
                input.wrap = Some(state.rand_mut().next() as i16);
                Ok(MutationResult::Mutated)
            }
            Some(_e) => Ok(MutationResult::Skipped),
        }
    }
}

impl Named for Base64FlipWrapMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("Base64FlipWrapMutator");
        &NAME
    }
}
pub struct Base64RawDataMutator;
impl<S> Mutator<Base64Input, S> for Base64RawDataMutator
where
    S: HasRand + HasMaxSize + HasCorpus<Input = Base64Input>,
{
    fn mutate(&mut self, state: &mut S, input: &mut Base64Input) -> Result<MutationResult, Error> {
        let index = state
            .rand_mut()
            .below(havoc_mutations::<Base64Input>().len());
        havoc_mutations().get_and_mutate(index.into(), state, input)
    }
}

impl Named for Base64RawDataMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("Base64RawDataMutator");
        &NAME
    }
}
