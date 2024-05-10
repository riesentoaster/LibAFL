use core::fmt;
use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    fmt::{Debug, Display, Formatter},
    hash::{DefaultHasher, Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use libafl::{
    generators::{Generator, RandPrintablesGenerator},
    inputs::{HasBytesVec, Input},
    mutators::{havoc_mutations, MutationResult, Mutator, MutatorsTuple},
    state::{HasCorpus, HasMaxSize, HasRand},
    Error, SerdeAny,
};

use libafl_bolts::{prelude::Rand, HasLen, Named};

use crate::{generic::ExtractsToCommand, metadata_structs::vec_string_mapper};

/// An [`Input`] implementation for coreutils' `base64`
#[derive(Serialize, Deserialize, Clone, Debug, Hash, SerdeAny)]
pub struct Base64Input {
    pub raw_data: Vec<u8>,
    pub decode: bool,
    pub ignore_garbage: bool,
    pub wrap: Option<i16>,
}

impl Display for Base64Input {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "input: '{}'",
            vec_string_mapper(&Some(self.raw_data.clone()))
        )?;
        if self.decode {
            write!(f, ", decode")?;
        }
        if self.ignore_garbage {
            write!(f, ", ignore_garbage")?;
        }
        if let Some(w) = self.wrap {
            write!(f, ", wrap: {}", w)?;
        }
        Ok(())
    }
}

impl Input for Base64Input {
    #[must_use]
    fn generate_name(&self, _idx: usize) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn wrapped_as_testcase(&mut self) {}
}

impl HasBytesVec for Base64Input {
    #[must_use]
    fn bytes(&self) -> &[u8] {
        &self.raw_data
    }

    #[must_use]
    fn bytes_mut(&mut self) -> &mut Vec<u8> {
        &mut self.raw_data
    }
}

impl ExtractsToCommand for Base64Input {
    #[must_use]
    fn get_stdin(&self) -> &Vec<u8> {
        &self.raw_data
    }

    #[must_use]
    fn get_args<'a>(&self) -> Vec<Cow<'a, OsStr>> {
        let mut args = Vec::with_capacity(4);
        if self.decode {
            args.push(Cow::Borrowed(OsStr::new("-d")))
        }
        if self.ignore_garbage {
            args.push(Cow::Borrowed(OsStr::new("-i")))
        }
        if let Some(w) = self.wrap {
            args.push(Cow::Borrowed(OsStr::new("-w")));
            args.push(Cow::Owned(OsString::from(w.to_string())))
        }
        args
    }
}

impl Base64Input {
    #[must_use]
    pub fn new(raw_data: &[u8], decode: bool, ignore_garbage: bool, wrap: Option<i16>) -> Self {
        Self {
            raw_data: Vec::from(raw_data),
            decode,
            ignore_garbage,
            wrap,
        }
    }
}

pub struct Base64Generator {
    max_size: usize,
}

impl<S> Generator<Base64Input, S> for Base64Generator
where
    S: HasRand,
{
    fn generate(&mut self, state: &mut S) -> Result<Base64Input, Error> {
        let binding = RandPrintablesGenerator::new(self.max_size).generate(state)?;
        let raw_data = binding.bytes();

        let rand = state.rand_mut();
        let decode = rand.coinflip(0.5);
        let ignore_garbage = rand.coinflip(0.5);
        let wrap = if rand.coinflip(0.5) {
            Some(rand.next() as i16)
        } else {
            None
        };
        Ok(Base64Input::new(raw_data, decode, ignore_garbage, wrap))
    }
}

impl Base64Generator {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

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
        &Cow::Borrowed("Base64FlipDecodeMutator")
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
        &Cow::Borrowed("Base64FlipIgnoreGarbageMutator")
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
        &Cow::Borrowed("Base64WrapContentMutator")
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
        &Cow::Borrowed("Base64FlipWrapMutator")
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
        &Cow::Borrowed("Base64RawDataMutator")
    }
}
