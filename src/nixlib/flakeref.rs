use std::str::FromStr;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct FlakeReference {
    pub flake_path: String,
    pub attribute: String,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    MultipleHashSigns,
}

impl FromStr for FlakeReference {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = input.split('#').collect();

        match parts.len() {
            0 => {
                Ok(FlakeReference {
                    flake_path: String::new(),
                    attribute: String::new(),
                })
            }
            1 => {
                Ok(FlakeReference {
                    flake_path: String::new(),
                    attribute: parts[0].to_string(),
                })
            }
            2 => {
                Ok(FlakeReference {
                    flake_path: parts[0].to_string(),
                    attribute: parts[1].to_string(),
                })
            }
            _ => Err(ParseError::MultipleHashSigns),
        }
    }
}

/// Implementing Display for ParseError to enable formatted error messages.
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::MultipleHashSigns => write!(f, "Multiple '#' signs found in input"),
        }
    }
}

/// Custom value parser to parse flake references from strings
pub fn parse_flake_reference(s: &str) -> Result<FlakeReference, String> {
    FlakeReference::from_str(s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_empty_string() {
        let parsed = FlakeReference::from_str("");
        let expected = Ok(FlakeReference {
            flake_path: "".to_string(),
            attribute: "".to_string(),
        });
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_no_flake_path() {
        let parsed = FlakeReference::from_str("bla");
        let expected = Ok(FlakeReference {
            flake_path: "".to_string(),
            attribute: "bla".to_string(),
        });
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_with_flake_path() {
        let parsed = FlakeReference::from_str("foo#bar");
        let expected = Ok(FlakeReference {
            flake_path: "foo".to_string(),
            attribute: "bar".to_string(),
        });
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_empty_flake_path_with_attribute() {
        let parsed = FlakeReference::from_str("#bar");
        let expected = Ok(FlakeReference {
            flake_path: "".to_string(),
            attribute: "bar".to_string(),
        });
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_flake_path_with_empty_attribute() {
        let parsed = FlakeReference::from_str("foo#");
        let expected = Ok(FlakeReference {
            flake_path: "foo".to_string(),
            attribute: "".to_string(),
        });
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_multiple_hash_signs_error() {
        let parsed = FlakeReference::from_str("foo#bar#baz");
        let expected = Err(ParseError::MultipleHashSigns);
        assert_eq!(parsed, expected);
    }
}
