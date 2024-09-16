use serde::Deserialize;
use std::str;
use std::collections::HashMap;

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

pub fn single_nix_build_output(stdout: &Vec<u8>) -> Result<String, OutputError> {
    let stdout_str = str::from_utf8(stdout).expect("Failed to convert to string");
    let build_outputs: Vec<BuildOutput> = serde_json::from_str(stdout_str)
        .map_err(|_| OutputError::DeserializationError)?;

    match build_outputs.len() {
        0 => Err(OutputError::NoOutputPath),
        1 => match build_outputs[0].outputs.get("out") {
            Some(out) => Ok(out.clone()),
            None => Err(OutputError::NoOutputPath)
        },
        _ => Err(OutputError::MultipleOutputPaths),
    }
}