use core::fmt;
use std::{
    borrow::Cow,
    ffi::OsStr,
    fmt::{Display, Formatter},
    hash::{DefaultHasher, Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use libafl::{
    generators::Generator,
    inputs::{Input, UsesInput},
    mutators::{
        BitFlipMutator, ByteAddMutator, ByteDecMutator, ByteFlipMutator, ByteIncMutator,
        ByteInterestingMutator, ByteNegMutator, ByteRandMutator, BytesCopyMutator,
        BytesDeleteMutator, BytesExpandMutator, BytesInsertCopyMutator, BytesInsertMutator,
        BytesRandInsertMutator, BytesRandSetMutator, BytesSetMutator, BytesSwapMutator, DwordAddMutator, DwordInterestingMutator,
        MutationResult, Mutator, QwordAddMutator, WordAddMutator, WordInterestingMutator,
    },
    state::{HasCorpus, HasMaxSize, HasRand},
    Error, SerdeAny,
};

use libafl_bolts::{
    prelude::Rand,
    tuples::{tuple_list, tuple_list_type},
    Named,
};

use crate::generic::{
    executor::{arg_from_vec, ExtractsToCommand},
    mutator::{MappingMutator, MappingOptionMutator},
    stdio::vec_string_mapper,
};

/// An [`Input`] implementation for coreutils' `base64`
#[derive(Serialize, Deserialize, Clone, Debug, Hash, SerdeAny)]
pub struct Base64Input {
    pub input: Vec<u8>,
    pub decode: bool,
    pub ignore_garbage: bool,
    pub wrap: Option<Vec<u8>>,
}

impl Display for Base64Input {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "input: '{}'",
            vec_string_mapper(&Some(self.input.clone()))
        )?;
        if self.decode {
            write!(f, ", decode")?;
        }
        if self.ignore_garbage {
            write!(f, ", ignore_garbage")?;
        }
        if self.wrap.is_some() {
            write!(f, ", wrap: {}", vec_string_mapper(&self.wrap))?;
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
}

impl ExtractsToCommand for Base64Input {
    #[must_use]
    fn get_stdin(&self) -> &Vec<u8> {
        &self.input
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
        if let Some(w) = &self.wrap {
            args.push(Cow::Borrowed(OsStr::new("-w")));
            args.push(Cow::Owned(arg_from_vec(w)))
        }
        args
    }
}

impl Base64Input {
    #[must_use]
    pub fn new(input: &[u8], decode: bool, ignore_garbage: bool, wrap: Option<Vec<u8>>) -> Self {
        Self {
            input: Vec::from(input),
            decode,
            ignore_garbage,
            wrap,
        }
    }
    pub fn extract_input(&mut self) -> &mut Vec<u8> {
        &mut self.input
    }
    pub fn extract_wrap(&mut self) -> &mut Option<Vec<u8>> {
        &mut self.wrap
    }
}

pub struct Base64Generator {
    input_size: u32,
    wrap_size: u32,
}

impl Base64Generator {
    pub fn new(input_size: u32, wrap_size: u32) -> Self {
        Self {
            input_size,
            wrap_size,
        }
    }
}

impl<S> Generator<Base64Input, S> for Base64Generator
where
    S: HasRand,
{
    fn generate(&mut self, state: &mut S) -> Result<Base64Input, Error> {
        let input = &generate_bytes(state, self.input_size);

        let rand = state.rand_mut();
        let decode = rand.coinflip(0.5);
        let ignore_garbage = rand.coinflip(0.5);
        let wrap = rand
            .coinflip(0.5)
            .then(|| generate_bytes(state, self.wrap_size));
        Ok(Base64Input::new(input, decode, ignore_garbage, wrap))
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

pub struct Base64FlipWrapMutator;
impl<S> Mutator<Base64Input, S> for Base64FlipWrapMutator
where
    S: HasRand,
{
    fn mutate(&mut self, state: &mut S, input: &mut Base64Input) -> Result<MutationResult, Error> {
        match &input.wrap {
            None => {
                input.wrap = Some(generate_bytes(state, 2));
                Ok(MutationResult::Mutated)
            }
            Some(_e) => {
                input.wrap = None;
                Ok(MutationResult::Mutated)
            }
        }
    }
}

impl Named for Base64FlipWrapMutator {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("Base64FlipWrapMutator")
    }
}

fn generate_bytes<S: HasRand>(state: &mut S, len: u32) -> Vec<u8> {
    (0..len)
        .map(|_e| state.rand_mut().below(u8::MAX as usize + 1) as u8)
        .collect::<Vec<_>>()
}

pub type Base64Mutators<'a, S> = tuple_list_type!(
    MappingMutator<S, BitFlipMutator>,
    MappingMutator<S, ByteFlipMutator>,
    MappingMutator<S, ByteIncMutator>,
    MappingMutator<S, ByteDecMutator>,
    MappingMutator<S, ByteNegMutator>,
    MappingMutator<S, ByteRandMutator>,
    MappingMutator<S, ByteAddMutator>,
    MappingMutator<S, WordAddMutator>,
    MappingMutator<S, DwordAddMutator>,
    MappingMutator<S, QwordAddMutator>,
    MappingMutator<S, ByteInterestingMutator>,
    MappingMutator<S, WordInterestingMutator>,
    MappingMutator<S, DwordInterestingMutator>,
    MappingMutator<S, BytesDeleteMutator>,
    MappingMutator<S, BytesDeleteMutator>,
    MappingMutator<S, BytesDeleteMutator>,
    MappingMutator<S, BytesDeleteMutator>,
    MappingMutator<S, BytesExpandMutator>,
    MappingMutator<S, BytesInsertMutator>,
    MappingMutator<S, BytesRandInsertMutator>,
    MappingMutator<S, BytesSetMutator>,
    MappingMutator<S, BytesRandSetMutator>,
    MappingMutator<S, BytesCopyMutator>,
    MappingMutator<S, BytesInsertCopyMutator>,
    MappingMutator<S, BytesSwapMutator>,
    MappingOptionMutator<S, BitFlipMutator>,
    MappingOptionMutator<S, ByteFlipMutator>,
    MappingOptionMutator<S, ByteIncMutator>,
    MappingOptionMutator<S, ByteDecMutator>,
    MappingOptionMutator<S, ByteNegMutator>,
    MappingOptionMutator<S, ByteRandMutator>,
    MappingOptionMutator<S, ByteAddMutator>,
    MappingOptionMutator<S, WordAddMutator>,
    MappingOptionMutator<S, DwordAddMutator>,
    MappingOptionMutator<S, QwordAddMutator>,
    MappingOptionMutator<S, ByteInterestingMutator>,
    MappingOptionMutator<S, WordInterestingMutator>,
    MappingOptionMutator<S, DwordInterestingMutator>,
    MappingOptionMutator<S, BytesDeleteMutator>,
    MappingOptionMutator<S, BytesDeleteMutator>,
    MappingOptionMutator<S, BytesDeleteMutator>,
    MappingOptionMutator<S, BytesDeleteMutator>,
    MappingOptionMutator<S, BytesExpandMutator>,
    MappingOptionMutator<S, BytesInsertMutator>,
    MappingOptionMutator<S, BytesRandInsertMutator>,
    MappingOptionMutator<S, BytesSetMutator>,
    MappingOptionMutator<S, BytesRandSetMutator>,
    MappingOptionMutator<S, BytesCopyMutator>,
    MappingOptionMutator<S, BytesInsertCopyMutator>,
    MappingOptionMutator<S, BytesSwapMutator>,
    Base64FlipDecodeMutator,
    Base64FlipIgnoreGarbageMutator,
    Base64FlipWrapMutator,
);

pub fn base64_mutators<'a, S>() -> Base64Mutators<'a, S>
where
    S: UsesInput<Input = Base64Input> + HasRand + HasMaxSize + HasCorpus,
{
    tuple_list!(
        MappingMutator::new(Base64Input::extract_input, BitFlipMutator::new()),
        MappingMutator::new(Base64Input::extract_input, ByteFlipMutator::new()),
        MappingMutator::new(Base64Input::extract_input, ByteIncMutator::new()),
        MappingMutator::new(Base64Input::extract_input, ByteDecMutator::new()),
        MappingMutator::new(Base64Input::extract_input, ByteNegMutator::new()),
        MappingMutator::new(Base64Input::extract_input, ByteRandMutator::new()),
        MappingMutator::new(Base64Input::extract_input, ByteAddMutator::new()),
        MappingMutator::new(Base64Input::extract_input, WordAddMutator::new()),
        MappingMutator::new(Base64Input::extract_input, DwordAddMutator::new()),
        MappingMutator::new(Base64Input::extract_input, QwordAddMutator::new()),
        MappingMutator::new(Base64Input::extract_input, ByteInterestingMutator::new()),
        MappingMutator::new(Base64Input::extract_input, WordInterestingMutator::new()),
        MappingMutator::new(Base64Input::extract_input, DwordInterestingMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesDeleteMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesDeleteMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesDeleteMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesDeleteMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesExpandMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesInsertMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesRandInsertMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesSetMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesRandSetMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesCopyMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesInsertCopyMutator::new()),
        MappingMutator::new(Base64Input::extract_input, BytesSwapMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BitFlipMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, ByteFlipMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, ByteIncMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, ByteDecMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, ByteNegMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, ByteRandMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, ByteAddMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, WordAddMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, DwordAddMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, QwordAddMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, ByteInterestingMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, WordInterestingMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, DwordInterestingMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesDeleteMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesDeleteMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesDeleteMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesDeleteMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesExpandMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesInsertMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesRandInsertMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesSetMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesRandSetMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesCopyMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesInsertCopyMutator::new()),
        MappingOptionMutator::new(Base64Input::extract_wrap, BytesSwapMutator::new()),
        Base64FlipDecodeMutator,
        Base64FlipIgnoreGarbageMutator,
        Base64FlipWrapMutator,
    )
}
