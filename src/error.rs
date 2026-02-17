use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("EC error: {0}")]
    Ec(String),

    #[error("Socket error: {0}")]
    Socket(String),

    #[error("Strategy error: {0}")]
    Strategy(String),

    #[error("Invalid command: {0}")]
    Command(String),
}

pub type Result<T> = std::result::Result<T, Error>;
