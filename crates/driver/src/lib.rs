use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    triple: String,
}

impl Target {
    pub fn host() -> Self {
        Self {
            triple: "x86_64-apple-darwin".to_owned(),
        }
    }

    pub fn parse(triple: &str) -> Result<Self, TargetError> {
        let valid = [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
        ];

        if valid.contains(&triple) {
            Ok(Self {
                triple: triple.to_owned(),
            })
        } else {
            Err(TargetError {
                triple: triple.to_owned(),
            })
        }
    }

    pub fn triple(&self) -> &str {
        &self.triple
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetError {
    triple: String,
}

impl fmt::Display for TargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported target triple: {}", self.triple)
    }
}

impl std::error::Error for TargetError {}

#[cfg(test)]
mod tests {
    use super::Target;

    #[test]
    fn accepts_documented_target_triples() {
        assert_eq!(
            Target::parse("x86_64-apple-darwin").unwrap().triple(),
            "x86_64-apple-darwin"
        );
    }

    #[test]
    fn rejects_unknown_target_triples() {
        assert!(Target::parse("wasm32-unknown-unknown").is_err());
    }
}
