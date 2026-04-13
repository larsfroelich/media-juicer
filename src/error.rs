use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum MediaJuicerError {
    Io(std::io::Error),
    InvalidInput(&'static str),
}

impl Display for MediaJuicerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

impl std::error::Error for MediaJuicerError {}

impl From<std::io::Error> for MediaJuicerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, MediaJuicerError>;

#[cfg(test)]
mod tests {
    use super::MediaJuicerError;

    #[test]
    fn invalid_input_error_displays_message() {
        let error = MediaJuicerError::InvalidInput("missing path");
        assert_eq!(error.to_string(), "invalid input: missing path");
    }
}
