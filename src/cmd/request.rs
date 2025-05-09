use crate::{
    cmd::{
        error::ClientError,
        execution::arithmetic::Integer,
        parser::{arithmetic::Integer as IntegerParser, set::Set as SetParser},
        response::Response,
        types::{DECR, DECRBY, DEL, ECHO, EXISTS, GET, INCR, INCRBY, PING, SET},
    },
    db::{Db, Object},
};

#[derive(Debug, PartialEq)]
pub enum Request {
    Ping(Option<String>),
    Echo(String),
    Get(String),
    Set(SetParser),
    Exists(Vec<String>),
    Del(Vec<String>),
    Incr(String),
    Decr(String),
    IncrBy(IntegerParser),
    DecrBy(IntegerParser),
}

impl Request {
    pub fn execute(self, db: &Db) -> Response {
        match self {
            Self::Ping(val) => val.map_or(
                Response::SimpleString("PONG".to_string()),
                Response::BulkString,
            ),

            Self::Echo(val) => Response::BulkString(val),

            Self::Set(parser) => {
                let mut map = db.lock().unwrap();
                map.insert(parser.key, Object::new(parser.value, parser.expiration));
                Response::SimpleString("OK".to_string())
            }

            Self::Get(key) => {
                let mut map = db.lock().unwrap();

                match map.get(&key) {
                    None => Response::Null,
                    Some(o) if o.is_expired() => {
                        map.swap_remove(&key);
                        Response::Null
                    }
                    Some(o) => Response::BulkString(o.value.to_string()),
                }
            }

            Self::Exists(keys) => {
                let mut map = db.lock().unwrap();
                let mut existing_keys = 0u64;

                for k in keys {
                    if let Some(o) = map.get(&k) {
                        if o.is_expired() {
                            map.swap_remove(&k);
                        } else {
                            existing_keys += 1;
                        }
                    }
                }

                Response::Integer(existing_keys.to_string())
            }

            Self::Del(keys) => {
                let mut map = db.lock().unwrap();
                let mut deleted_keys = 0u64;

                for k in keys {
                    if map.swap_remove(&k).is_some() {
                        deleted_keys += 1;
                    }
                }

                Response::Integer(deleted_keys.to_string())
            }

            Self::Incr(key) => {
                Integer::Incr
                    .execute(db, key)
                    .map_or_else(
                        |e| Response::SimpleError(e.to_string()),
                        |v| Response::Integer(v.to_string())
                    )
            },

            Self::Decr(key) => {
                Integer::Decr
                    .execute(db, key)
                    .map_or_else(
                        |e| Response::SimpleError(e.to_string()),
                        |v| Response::Integer(v.to_string())
                    )
            }

            Self::IncrBy(parser) => {
                Integer::IncrBy(parser.value)
                    .execute(db, parser.key)
                    .map_or_else(
                        |e| Response::SimpleError(e.to_string()),
                        |v| Response::Integer(v.to_string())
                    )
            }

            Self::DecrBy(parser) => {
                Integer::DecrBy(parser.value)
                    .execute(db, parser.key)
                    .map_or_else(
                        |e| Response::SimpleError(e.to_string()),
                        |v| Response::Integer(v.to_string())
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
            PING => {
                if params.len() > 2 {
                    Err(ClientError::WrongNumberOfArguments(PING.to_string()))
                } else {
                    Ok(Request::Ping(params.get(1).cloned()))
                }
            }

            ECHO => {
                if params.len() != 2 {
                    Err(ClientError::WrongNumberOfArguments(ECHO.to_string()))
                } else {
                    Ok(Request::Echo(params[1].to_owned()))
                }
            }

            SET => {
                if params.len() == 1 {
                    Err(ClientError::WrongNumberOfArguments(SET.to_string()))
                } else {
                    Ok(SetParser::parse(&params[1..]).map(Request::Set))?
                }
            }

            GET => {
                if params.len() != 2 {
                    Err(ClientError::WrongNumberOfArguments(GET.to_string()))
                } else {
                    Ok(Request::Get(params[1].to_owned()))
                }
            }

            EXISTS => {
                if params.len() < 2 {
                    Err(ClientError::WrongNumberOfArguments(EXISTS.to_string()))
                } else {
                    Ok(Request::Exists(params[1..].to_vec()))
                }
            }

            DEL => {
                if params.len() < 2 {
                    Err(ClientError::WrongNumberOfArguments(DEL.to_string()))
                } else {
                    Ok(Request::Del(params[1..].to_vec()))
                }
            }

            INCR => {
                if params.len() != 2 {
                    Err(ClientError::WrongNumberOfArguments(INCR.to_string()))
                } else {
                    Ok(Request::Incr(params[1].to_owned()))
                }
            }

            DECR => {
                if params.len() != 2 {
                    Err(ClientError::WrongNumberOfArguments(DECR.to_string()))
                } else {
                    Ok(Request::Decr(params[1].to_owned()))
                }
            }

            INCRBY => {
                if params.len() == 1 {
                    Err(ClientError::WrongNumberOfArguments(INCRBY.to_string()))
                } else {
                    Ok(IntegerParser::parse(&params[1..]).map(Request::IncrBy))?
                }
            }

            DECRBY => {
                if params.len() == 1 {
                    Err(ClientError::WrongNumberOfArguments(DECRBY.to_string()))
                } else {
                    Ok(IntegerParser::parse(&params[1..]).map(Request::DecrBy))?
                }
            }

            c => Err(ClientError::UnknownCommand(c.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Mutex,
        time::{Duration, SystemTime},
    };

    use indexmap::IndexMap;

    use crate::db::Value;

    use super::*;

    #[test]
    fn decrby_one_arg() {
        let params = vec![DECRBY.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(DECRBY.to_string())
        );
    }

    #[test]
    fn decrby_ok() {
        let params = vec![DECRBY.to_string(), "key".to_string(), 100.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap(),
            Request::DecrBy(IntegerParser {
                key: "key".to_string(),
                value: 100,
            })
        );
    }

    #[test]
    fn incrby_one_arg() {
        let params = vec![INCRBY.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(INCRBY.to_string())
        );
    }

    #[test]
    fn incrby_ok() {
        let params = vec![INCRBY.to_string(), "key".to_string(), 100.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap(),
            Request::IncrBy(IntegerParser {
                key: "key".to_string(),
                value: 100,
            })
        );
    }

    #[test]
    fn decr_one_arg() {
        let params = vec![DECR.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(DECR.to_string())
        );
    }

    #[test]
    fn decr_multiple_args() {
        let params = vec![DECR.to_string(), "key".to_string(), "key2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(DECR.to_string())
        );
    }

    #[test]
    fn decr_ok() {
        let params = vec![DECR.to_string(), "key".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Decr("key".to_string()));
    }

    #[test]
    fn incr_one_arg() {
        let params = vec![INCR.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(INCR.to_string())
        );
    }

    #[test]
    fn incr_multiple_args() {
        let params = vec![INCR.to_string(), "key".to_string(), "key2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(INCR.to_string())
        );
    }

    #[test]
    fn incr_ok() {
        let params = vec![INCR.to_string(), "key".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Incr("key".to_string()));
    }

    #[test]
    fn del_one_arg() {
        let params = vec![DEL.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(DEL.to_string())
        );
    }

    #[test]
    fn del_ok() {
        let params = vec![DEL.to_string(), "key".to_string(), "key2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap(),
            Request::Del(vec!["key".to_string(), "key2".to_string()])
        );
    }

    #[test]
    fn exists_one_arg() {
        let params = vec![EXISTS.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(EXISTS.to_string())
        );
    }

    #[test]
    fn exists_ok() {
        let params = vec![EXISTS.to_string(), "key".to_string(), "key2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap(),
            Request::Exists(vec!["key".to_string(), "key2".to_string()])
        );
    }

    #[test]
    fn set_one_arg() {
        let params = vec![SET.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(SET.to_string())
        );
    }

    #[test]
    fn set_ok() {
        let params = vec![SET.to_string(), "key".to_string(), "".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap(),
            Request::Set(SetParser {
                key: "key".to_string(),
                value: Value::String("".to_string()),
                expiration: None
            })
        );
    }

    #[test]
    fn get_no_args() {
        let params = vec![GET.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(GET.to_string())
        );
    }

    #[test]
    fn get_ok() {
        let params = vec![GET.to_string(), "key".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Get("key".to_string()));
    }

    #[test]
    fn ping_no_args() {
        let params = vec![PING.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Ping(None));
    }

    #[test]
    fn ping_with_arg() {
        let params = vec![PING.to_string(), "hello".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Ping(Some("hello".to_string())));
    }

    #[test]
    fn ping_too_many_args() {
        let params = vec![PING.to_string(), "arg1".to_string(), "arg2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(PING.to_string())
        );
    }

    #[test]
    fn echo_ok() {
        let params = vec![ECHO.to_string(), "hello world".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(cmd.unwrap(), Request::Echo("hello world".to_string()));
    }

    #[test]
    fn echo_no_args() {
        let params = vec![ECHO.to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(ECHO.to_string())
        );
    }

    #[test]
    fn echo_too_many_args() {
        let params = vec![ECHO.to_string(), "arg1".to_string(), "arg2".to_string()];
        let cmd = Request::try_from(params);
        assert_eq!(
            cmd.unwrap_err(),
            ClientError::WrongNumberOfArguments(ECHO.to_string())
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
        let ping_lowercase = vec![PING.to_string()];
        let ping_uppercase = vec!["PING".to_string()];
        let ping_mixed_case = vec!["PiNg".to_string()];

        assert!(Request::try_from(ping_lowercase).is_ok());
        assert!(Request::try_from(ping_uppercase).is_ok());
        assert!(Request::try_from(ping_mixed_case).is_ok());
    }

    #[test]
    fn execute_ping_no_arg() {
        let cmd = Request::Ping(None);
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::SimpleString("PONG".to_string()));
    }

    #[test]
    fn execute_ping_arg() {
        let cmd = Request::Ping(Some("ciao".to_string()));
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::BulkString("ciao".to_string()));
    }

    #[test]
    fn execute_ping_with_arg() {
        let cmd = Request::Ping(Some("hello".to_string()));
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::BulkString("hello".to_string()));
    }

    #[test]
    fn execute_echo() {
        let cmd = Request::Echo("test message".to_string());
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::BulkString("test message".to_string()));
    }

    #[test]
    fn execute_set_ok() {
        let set = SetParser {
            key: "key".to_string(),
            value: Value::String("".to_string()),
            expiration: None,
        };
        let cmd = Request::Set(set);
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::SimpleString("OK".to_string()));
    }

    #[test]
    fn execute_get_null() {
        let cmd = Request::Get("key".to_string());
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::Null);
    }

    #[test]
    fn execute_get_no_expiration() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        let cmd = Request::Get("key".to_string());
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::BulkString("value".to_string()));
    }

    #[test]
    fn execute_get_expired() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), Some(SystemTime::now())),
        );
        let cmd = Request::Get("key".to_string());
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Null);
    }

    #[test]
    fn execute_get_not_expired() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(
                Value::String("value".to_string()),
                Some(
                    SystemTime::now()
                        .checked_add(Duration::from_secs(10))
                        .unwrap(),
                ),
            ),
        );
        let cmd = Request::Get("key".to_string());
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::BulkString("value".to_string()));
    }

    #[test]
    fn execute_exists_zero() {
        let cmd = Request::Exists(vec!["key".to_string()]);
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::Integer("0".to_string()));
    }

    #[test]
    fn execute_exists_no_expiration() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        let cmd = Request::Exists(vec!["key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_exists_same_key_twice() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        let cmd = Request::Exists(vec!["key".to_string(), "key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("2".to_string()));
    }

    #[test]
    fn execute_exists_not_expired() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(
                Value::String("value".to_string()),
                Some(
                    SystemTime::now()
                        .checked_add(Duration::from_secs(100))
                        .unwrap(),
                ),
            ),
        );
        let cmd = Request::Exists(vec!["key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_exists_expired() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), Some(SystemTime::now())),
        );
        let cmd = Request::Exists(vec!["key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("0".to_string()));
    }

    #[test]
    fn execute_exists_multiple_keys() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        db.lock().unwrap().insert(
            "key2".to_string(),
            Object::new(Value::String("".to_string()), None),
        );
        let cmd = Request::Exists(vec!["key".to_string(), "key2".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("2".to_string()));
    }

    #[test]
    fn execute_exists_multiple_keys_one_expired() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        db.lock().unwrap().insert(
            "key2".to_string(),
            Object::new(Value::String("".to_string()), Some(SystemTime::now())),
        );
        let cmd = Request::Exists(vec!["key".to_string(), "key2".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_del_zero() {
        let cmd = Request::Del(vec!["key".to_string()]);
        let reply = cmd.execute(&Db::new(Mutex::new(IndexMap::new())));
        assert_eq!(reply, Response::Integer("0".to_string()));
    }

    #[test]
    fn execute_del_one() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        let cmd = Request::Del(vec!["key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_del_one_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        let cmd = Request::Del(vec!["key".to_string(), "key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_del_multiple() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new(Value::String("value".to_string()), None),
        );
        db.lock().unwrap().insert(
            "key2".to_string(),
            Object::new(Value::String("".to_string()), Some(SystemTime::now())),
        );
        let cmd = Request::Del(vec!["key".to_string(), "key2".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("2".to_string()));
    }

    #[test]
    fn execute_incr_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let cmd = Request::Incr("counter".to_string());
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_incr_err() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".to_string(),
            Object::new(Value::String("foo".to_string()), None),
        );
        let cmd = Request::Incr("counter".to_string());
        let reply = cmd.execute(&db);
        assert!(matches!(reply, Response::SimpleError(_)));
    }

    #[test]
    fn execute_decr_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let cmd = Request::Decr("counter".to_string());
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("-1".to_string()));
    }

    #[test]
    fn execute_decr_err() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".to_string(),
            Object::new(Value::String("foo".to_string()), None),
        );
        let cmd = Request::Decr("counter".to_string());
        let reply = cmd.execute(&db);
        assert!(matches!(reply, Response::SimpleError(_)));
    }

    #[test]
    fn execute_incrby_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let cmd = Request::IncrBy(IntegerParser { key: "counter".to_string(), value: 100 });
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("100".to_string()));
    }

    #[test]
    fn execute_incrby_err() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".to_string(),
            Object::new(Value::String("foo".to_string()), None),
        );
        let cmd = Request::IncrBy(IntegerParser { key: "counter".to_string(), value: 100 });
        let reply = cmd.execute(&db);
        assert!(matches!(reply, Response::SimpleError(_)));
    }

    #[test]
    fn execute_decrby_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let cmd = Request::DecrBy(IntegerParser { key: "counter".to_string(), value: 100 });
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("-100".to_string()));
    }

    #[test]
    fn execute_decrby_err() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".to_string(),
            Object::new(Value::String("foo".to_string()), None),
        );
        let cmd = Request::DecrBy(IntegerParser { key: "counter".to_string(), value: 100 });
        let reply = cmd.execute(&db);
        assert!(matches!(reply, Response::SimpleError(_)));
    }
}
