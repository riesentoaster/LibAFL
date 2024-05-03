use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
};

use libafl::inputs::{HasBytesVec, Input};
use serde::{Deserialize, Serialize};

use crate::executor::ExtractsToCommand;

/// An [`Input`] implementation for coreutils' `base64`
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Base64Input {
    pub raw_data: Vec<u8>,
    pub decode: bool,
    pub ignore_garbage: bool,
    pub wrap: Option<i16>,
}

impl Input for Base64Input {
    #[must_use]
    fn generate_name(&self, idx: usize) -> String {
        format!("{idx} — {self:?}")
    }
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
