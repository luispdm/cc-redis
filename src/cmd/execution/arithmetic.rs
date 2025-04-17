use crate::{
    cmd::error::ClientError,
    db::{Db, Object, Value},
};

pub enum Integer {
    Incr,
    Decr,
}

impl Integer {
    pub fn execute(&self, db: &Db, key: String) -> Result<i64, ClientError> {
        let mut map = db.lock().unwrap();
        let (initial_value, operation) = self.operation();

        match map.get(&key) {
            None => {
                map.insert(key, Object::new(Value::Integer(initial_value), None));
                Ok(initial_value)
            }

            Some(obj) if obj.is_expired() => {
                map.swap_remove(&key);
                map.insert(key, Object::new(Value::Integer(initial_value), None));

                Ok(initial_value)
            }

            Some(obj) => match obj.value {
                Value::Integer(i) => {
                    // value moved to avoid borrowing issues
                    let exp = obj.expiration;

                    operation(i).map_or(
                        Err(ClientError::OverflowError),
                        |v| {
                            map.insert(key, Object::new(Value::Integer(v), exp));
                            Ok(v)
                        },
                    )
                }
                _ => Err(ClientError::IntegerError),
            },
        }
    }

    fn operation(&self) -> (i64, Box<dyn Fn(i64) -> Option<i64>>) {
        match self {
            Integer::Incr => (1, Box::new(|i: i64| i.checked_add(1))),
            Integer::Decr => (-1, Box::new(|i: i64| i.checked_sub(1))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::Mutex,
        time::{Duration, SystemTime},
    };
    use indexmap::IndexMap;
    use crate::db::{Value, Object};

    #[test]
    fn incr_new_key() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let result = Integer::Incr.execute(&db, "counter".into());
        assert_eq!(result, Ok(1));
    }

    #[test]
    fn incr_expired_key() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(
                Value::Integer(5),
                Some(SystemTime::now() - Duration::from_secs(10))
            ),
        );
        let result = Integer::Incr.execute(&db, "counter".into());
        assert_eq!(result, Ok(1));
    }

    #[test]
    fn incr_valid_integer() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(5), None)
        );
        let result = Integer::Incr.execute(&db, "counter".into());
        assert_eq!(result, Ok(6));
    }

    #[test]
    fn incr_overflow() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(i64::MAX), None)
        );
        let result = Integer::Incr.execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::OverflowError));
    }

    #[test]
    fn incr_non_integer_value() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::String("foo".into()), None)
        );
        let result = Integer::Incr.execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::IntegerError));
    }

    #[test]
    fn incr_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        assert_eq!(Integer::Incr.execute(&db, "counter".into()), Ok(1));
        assert_eq!(Integer::Incr.execute(&db, "counter".into()), Ok(2));
    }

    #[test]
    fn decr_new_key() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let result = Integer::Decr.execute(&db, "counter".into());
        assert_eq!(result, Ok(-1));
    }

    #[test]
    fn decr_expired_key() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(
                Value::Integer(5),
                Some(SystemTime::now() - Duration::from_secs(10))
            ),
        );
        let result = Integer::Decr.execute(&db, "counter".into());
        assert_eq!(result, Ok(-1));
    }

    #[test]
    fn decr_valid_integer() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(5), None)
        );
        let result = Integer::Decr.execute(&db, "counter".into());
        assert_eq!(result, Ok(4));
    }

    #[test]
    fn decr_underflow() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(i64::MIN), None)
        );
        let result = Integer::Decr.execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::OverflowError));
    }

    #[test]
    fn decr_non_integer_value() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::String("foo".into()), None)
        );
        let result = Integer::Decr.execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::IntegerError));
    }

    #[test]
    fn decr_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        assert_eq!(Integer::Decr.execute(&db, "counter".into()), Ok(-1));
        assert_eq!(Integer::Decr.execute(&db, "counter".into()), Ok(-2));
    }
}
