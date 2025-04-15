use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ClientError {
    #[error("unknown command '{0}'")]
    UnknownCommand(String),
    #[error("wrong number of arguments for '{0}' command")]
    WrongNumberOfArguments(String),
    #[error("syntax error")]
    SyntaxError,
    #[error("value is not an integer or out of range")]
    IntegerError,
    #[error("increment or decrement would overflow")]
    OverflowError,
}
