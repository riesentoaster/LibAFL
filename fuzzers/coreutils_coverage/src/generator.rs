use libafl::{
    generators::{Generator, RandBytesGenerator},
    inputs::HasBytesVec,
    state::HasRand,
    Error,
};
use libafl_bolts::prelude::Rand;

use crate::input::Base64Input;

pub struct Base64Generator<'a> {
    max_size: usize,
    util: &'a str,
}
impl<'a, S> Generator<Base64Input, S> for Base64Generator<'a>
where
    S: HasRand,
{
    fn generate(&mut self, state: &mut S) -> Result<Base64Input, Error> {
        let binding = RandBytesGenerator::new(self.max_size).generate(state)?;
        let raw_data = binding.bytes();

        let rand = state.rand_mut();
        let decode = rand.coinflip(0.5);
        let ignore_garbage = rand.coinflip(0.5);
        let wrap = if rand.coinflip(0.5) {
            Some(rand.next() as i16)
        } else {
            None
        };
        Ok(Base64Input::new(
            raw_data,
            decode,
            ignore_garbage,
            wrap,
            self.util,
        ))
    }
}

impl<'a> Base64Generator<'a> {
    pub fn new(max_size: usize, util: &'a str) -> Self {
        Self { max_size, util }
    }
}
