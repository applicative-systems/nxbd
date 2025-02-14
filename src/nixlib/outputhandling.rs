#[allow(clippy::module_name_repetitions)]
pub fn single_nix_build_output(output: &[u8]) -> Result<String, serde_json::Error> {
    let parsed: Vec<String> = serde_json::from_slice(output)?;
    Ok(parsed.into_iter().next().expect("Empty build output"))
}
