use crate::{
    cmd::{
        error::ClientError,
        parser::set::Set,
        response::Response,
        types::{DEL, ECHO, EXISTS, GET, PING, SET},
    },
    db::{Db, ExpirationStatus, Object},
};

#[derive(Debug, PartialEq)]
pub enum Request {
    Ping(Option<String>),
    Echo(String),
    Get(String),
    Set(Set),
    Exists(Vec<String>),
    Del(Vec<String>),
}

impl Request {
    pub fn execute(self, db: &Db) -> Response {
        match self {
            Self::Ping(val) => val.map_or(
                Response::SimpleString("PONG".to_string()),
                Response::BulkString,
            ),
            Self::Echo(val) => Response::BulkString(val),
            Self::Set(set) => {
                let mut map = db.lock().unwrap();
                map.insert(set.key, Object::new(set.value, set.expiration));
                Response::SimpleString("OK".to_string())
            }
            Self::Get(key) => {
                let mut map = db.lock().unwrap();

                match ExpirationStatus::get(map.get(&key)) {
                    ExpirationStatus::NotExist => Response::Null,
                    ExpirationStatus::NotExpired(obj) => {
                        Response::BulkString(obj.value.to_string())
                    }
                    ExpirationStatus::Expired => {
                        map.swap_remove(&key);
                        Response::Null
                    }
                }
            }
            Self::Exists(keys) => {
                let mut map = db.lock().unwrap();
                let mut existing_keys = 0u64;

                for k in keys {
                    match ExpirationStatus::get(map.get(&k)) {
                        ExpirationStatus::NotExist => continue,
                        ExpirationStatus::NotExpired(_) => existing_keys += 1,
                        ExpirationStatus::Expired => {
                            map.swap_remove(&k);
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
                    Ok(Set::parse(params[1..].to_vec()).map(Request::Set))?
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

    use super::*;

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
            Request::Set(Set {
                key: "key".to_string(),
                value: "".to_string(),
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
        let set = Set {
            key: "key".to_string(),
            value: "".to_string(),
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
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
        let cmd = Request::Get("key".to_string());
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::BulkString("value".to_string()));
    }

    #[test]
    fn execute_get_expired() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "key".to_string(),
            Object::new("value".to_string(), Some(SystemTime::now())),
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
                "value".to_string(),
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
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
        let cmd = Request::Exists(vec!["key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_exists_same_key_twice() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
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
                "value".to_string(),
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
            Object::new("value".to_string(), Some(SystemTime::now())),
        );
        let cmd = Request::Exists(vec!["key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("0".to_string()));
    }

    #[test]
    fn execute_exists_multiple_keys() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
        db.lock()
            .unwrap()
            .insert("key2".to_string(), Object::new("".to_string(), None));
        let cmd = Request::Exists(vec!["key".to_string(), "key2".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("2".to_string()));
    }

    #[test]
    fn execute_exists_multiple_keys_one_expired() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
        db.lock().unwrap().insert(
            "key2".to_string(),
            Object::new("".to_string(), Some(SystemTime::now())),
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
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
        let cmd = Request::Del(vec!["key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_del_one_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
        let cmd = Request::Del(vec!["key".to_string(), "key".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("1".to_string()));
    }

    #[test]
    fn execute_del_multiple() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock()
            .unwrap()
            .insert("key".to_string(), Object::new("value".to_string(), None));
        db.lock().unwrap().insert(
            "key2".to_string(),
            Object::new("".to_string(), Some(SystemTime::now())),
        );
        let cmd = Request::Del(vec!["key".to_string(), "key2".to_string()]);
        let reply = cmd.execute(&db);
        assert_eq!(reply, Response::Integer("2".to_string()));
    }
}
