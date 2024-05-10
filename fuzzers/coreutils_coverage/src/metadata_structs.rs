use libafl::SerdeAny;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, SerdeAny)]
pub struct StdOutDiffMetadata {
    name: String,
    o1_res: String,
    o2_res: String,
}

impl StdOutDiffMetadata {
    pub fn new(o1_res: String, o2_res: String) -> Self {
        Self {
            name: "stdout-diff".to_string(),
            o1_res,
            o2_res,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SerdeAny)]
pub struct StdErrDiffMetadata {
    name: String,
    o1_res: String,
    o2_res: String,
}

impl StdErrDiffMetadata {
    #[allow(dead_code)]
    pub fn new(o1_res: String, o2_res: String) -> Self {
        Self {
            name: "stderr-diff".to_string(),
            o1_res,
            o2_res,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SerdeAny)]
pub struct StdErrBinaryDiffMetadata {
    name: String,
    o1_res: String,
    o2_res: String,
}

impl StdErrBinaryDiffMetadata {
    pub fn new(o1_res: String, o2_res: String) -> Self {
        Self {
            name: "stderr-binary-diff".to_string(),
            o1_res,
            o2_res,
        }
    }
}

pub fn vec_string_mapper(v: &Option<Vec<u8>>) -> String {
    v.as_ref()
        .map(|v| {
            std::str::from_utf8(v.as_ref())
                .map_or(
                    serde_json::to_string(&v).map(|s| format!("utf8 error, bytes: {}", s)),
                    |s| Ok(s.to_string()),
                )
                .unwrap_or("Serialization error".to_string())
        })
        .unwrap_or("Did not observe anything".to_string())
}

#[derive(SerdeAny, Debug, Serialize, Deserialize)]
pub struct InputMetadata {
    name: String,
    input: String,
}

impl InputMetadata {
    pub fn new(input: String) -> Self {
        Self {
            name: "input-metadata".to_string(),
            input,
        }
    }
}
