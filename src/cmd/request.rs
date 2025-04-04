use crate::{cmd::response::Response, db::Db};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ClientError {
    #[error("unknown command '{0}'")]
    UnknownCommand(String),
    #[error("wrong number of arguments for '{0}' command")]
    WrongNumberOfArguments(String),
}

#[derive(Debug, PartialEq)]
pub enum Request {
    Ping(Option<String>),
    Echo(String),
    Get(String),
    Set(String, String),
}

impl Request {
    pub fn execute(self, db: &Db) -> Response {
        match self {
            Self::Ping(val) => val.map_or(
                Response::SimpleString("PONG".to_string()),
                Response::BulkString,
            ),
            Self::Echo(val) => Response::BulkString(val),
            Self::Set(key, val) => {
                let mut map = db.lock().unwrap();
                map.insert(key, val);
                Response::SimpleString("OK".to_string())
            }
            Self::Get(key) => {
                let map = db.lock().unwrap();
                map.get(&key).map_or_else(
                    || Response::Null,
                    |val| Response::BulkString(val.to_string()),
                )
            }
        }
    }
}

impl TryFrom<Vec<String>> for Request {
    type Error = ClientError;

    fn try_from(params: Vec<String>) -> Result<Self, Self::Error> {
        if params.is_empty() {
            return Err(ClientError::UnknownCommand("".to_string()));
        }

        match params[0].to_lowercase().as_str() {
            "ping" => {
                if params.len() > 2 {
                    Err(ClientError::WrongNumberOfArguments("ping".to_string()))
                } else {
                    Ok(Request::Ping(params.get(1).cloned()))
                }
            }
            "echo" => {
                if params.len() != 2 {
                    Err(ClientError::WrongNumberOfArguments("echo".to_string()))
                } else {
                    Ok(Request::Echo(params[1].to_owned()))
                }
            }
            "set" => {
                if params.len() != 3 {
                    Err(ClientError::WrongNumberOfArguments("set".to_string()))
                } else {
                    Ok(Request::Set(params[1].to_owned(), params[2].to_owned()))
                }
            }
            "get" => {
                if params.len() != 2 {
                    Err(ClientError::WrongNumberOfArguments("get".to_string()))
                } else {
                    Ok(Request::Get(params[1].to_owned()))
                }
            }
            c => Err(ClientError::UnknownCommand(c.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    #[test]
    fn get_no_args() {
        let params = vec!["GET".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap_err(), ClientError::WrongNumberOfArguments("get".to_string()));
    }

    #[test]
    fn get_ok() {
        let params = vec!["GET".to_string(), "key".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Get("key".to_string()));
    }

    #[test]
    fn ping_no_args() {
        let params = vec!["PING".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Ping(None));
    }

    #[test]
    fn ping_with_arg() {
        let params = vec!["PING".to_string(), "hello".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Ping(Some("hello".to_string())));
    }

    #[test]
    fn ping_too_many_args() {
        let params = vec!["PING".to_string(), "arg1".to_string(), "arg2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments("ping".to_string())
        );
    }

    #[test]
    fn echo_ok() {
        let params = vec!["ECHO".to_string(), "hello world".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Echo("hello world".to_string()));
    }

    #[test]
    fn echo_no_args() {
        let params = vec!["ECHO".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments("echo".to_string())
        );
    }

    #[test]
    fn echo_too_many_args() {
        let params = vec!["ECHO".to_string(), "arg1".to_string(), "arg2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments("echo".to_string())
        );
    }

    #[test]
    fn unknown_command() {
        let params = vec!["UNKNOWN".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::UnknownCommand("unknown".to_string())
        );
    }

    #[test]
    fn case_insensitive_commands() {
        let ping_lowercase = vec!["ping".to_string()];
        let ping_uppercase = vec!["PING".to_string()];
        let ping_mixed_case = vec!["PiNg".to_string()];

        assert!(Request::try_from(ping_lowercase).is_ok());
        assert!(Request::try_from(ping_uppercase).is_ok());
        assert!(Request::try_from(ping_mixed_case).is_ok());
    }

    #[test]
    fn execute_ping_no_arg() {
        let cmd = Request::Ping(None);
        let reply = cmd.execute(&Db::new(Mutex::new(HashMap::new())));
        assert_eq!(reply, Response::SimpleString("PONG".to_string()));
    }

    #[test]
    fn execute_ping_arg() {
        let cmd = Request::Ping(Some("ciao".to_string()));
        let reply = cmd.execute(&Db::new(Mutex::new(HashMap::new())));
        assert_eq!(reply, Response::BulkString("ciao".to_string()));
    }

    #[test]
    fn execute_ping_with_arg() {
        let cmd = Request::Ping(Some("hello".to_string()));
        let reply = cmd.execute(&Db::new(Mutex::new(HashMap::new())));
        assert_eq!(reply, Response::BulkString("hello".to_string()));
    }

    #[test]
    fn execute_echo() {
        let cmd = Request::Echo("test message".to_string());
        let reply = cmd.execute(&Db::new(Mutex::new(HashMap::new())));
        assert_eq!(reply, Response::BulkString("test message".to_string()));
    }

    #[test]
    fn execute_get_null() {
        let cmd = Request::Get("key".to_string());
        let reply = cmd.execute(&Db::new(Mutex::new(HashMap::new())));
        assert_eq!(reply, Response::Null);
    }

    #[test]
    fn execute_get_value() {
        let db = Db::new(Mutex::new(HashMap::new()));
        db.lock().unwrap().insert("key".to_string(), "value".to_string());
        let cmd = Request::Get("key".to_string());
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::BulkString("value".to_string()));
    }
}
