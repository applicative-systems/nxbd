use serde::Deserialize;
use std::collections::HashMap;
use std::str;

#[derive(Debug)]
pub enum OutputError {
    MultipleOutputPaths,
    NoOutputPath,
    DeserializationError,
}

#[derive(Debug, Deserialize)]
struct BuildOutput {
    outputs: HashMap<String, String>,
}

#[allow(clippy::module_name_repetitions)]
pub fn single_nix_build_output(output: &[u8]) -> Result<String, serde_json::Error> {
    let parsed: Vec<String> = serde_json::from_slice(output)?;
    Ok(parsed.into_iter().next().expect("Empty build output"))
}
