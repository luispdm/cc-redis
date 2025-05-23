use std::ops::Neg;

use crate::{
    cmd::error::ClientError,
    db::{Db, Object, Value},
};

pub enum Integer {
    Incr,
    Decr,
    IncrBy(i64),
    DecrBy(i64),
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

    pub fn operation(&self) -> (i64, Box<dyn Fn(i64) -> Option<i64> + '_>) {
        match self {
            Integer::Incr => (1, Box::new(|i: i64| i.checked_add(1))),
            Integer::Decr => (-1, Box::new(|i: i64| i.checked_sub(1))),
            Integer::IncrBy(v) => (*v, Box::new(|i: i64| i.checked_add(*v))),
            Integer::DecrBy(v) => (v.neg(), Box::new(|i: i64| i.checked_sub(*v))),
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
    fn incr_existing_integer() {
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
    fn decr_existing_integer() {
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

    #[test]
    fn incrby_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let result = Integer::IncrBy(100).execute(&db, "counter".into());
        assert_eq!(result, Ok(100));
    }

    #[test]
    fn incrby_negative_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let result = Integer::IncrBy(-100).execute(&db, "counter".into());
        assert_eq!(result, Ok(-100));
    }

    #[test]
    fn incrby_expired_key() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(
                Value::Integer(5),
                Some(SystemTime::now() - Duration::from_secs(10))
            ),
        );
        let result = Integer::IncrBy(10).execute(&db, "counter".into());
        assert_eq!(result, Ok(10));
    }

    #[test]
    fn incrby_existing_integer() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(5), None)
        );
        let result = Integer::IncrBy(10).execute(&db, "counter".into());
        assert_eq!(result, Ok(15));
    }

    #[test]
    fn incrby_negative_existing_integer() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(5), None)
        );
        let result = Integer::IncrBy(-10).execute(&db, "counter".into());
        assert_eq!(result, Ok(-5));
    }

    #[test]
    fn incrby_overflow() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(i64::MAX), None)
        );
        let result = Integer::IncrBy(100).execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::OverflowError));
    }

    #[test]
    fn incrby_underflow() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(i64::MIN), None)
        );
        let result = Integer::IncrBy(-100).execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::OverflowError));
    }

    #[test]
    fn incrby_non_integer_value() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::String("foo".into()), None)
        );
        let result = Integer::IncrBy(10).execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::IntegerError));
    }

    #[test]
    fn incrby_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        assert_eq!(Integer::IncrBy(10).execute(&db, "counter".into()), Ok(10));
        assert_eq!(Integer::IncrBy(10).execute(&db, "counter".into()), Ok(20));
    }

    #[test]
    fn incrby_negative_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        assert_eq!(Integer::DecrBy(10).execute(&db, "counter".into()), Ok(-10));
        assert_eq!(Integer::DecrBy(10).execute(&db, "counter".into()), Ok(-20));
    }

    #[test]
    fn decrby_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let result = Integer::DecrBy(100).execute(&db, "counter".into());
        assert_eq!(result, Ok(-100));
    }

    #[test]
    fn decrby_negative_ok() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        let result = Integer::DecrBy(-100).execute(&db, "counter".into());
        assert_eq!(result, Ok(100));
    }

    #[test]
    fn decrby_expired_key() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(
                Value::Integer(5),
                Some(SystemTime::now() - Duration::from_secs(10))
            ),
        );
        let result = Integer::DecrBy(10).execute(&db, "counter".into());
        assert_eq!(result, Ok(-10));
    }

    #[test]
    fn decrby_existing_integer() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(5), None)
        );
        let result = Integer::DecrBy(10).execute(&db, "counter".into());
        assert_eq!(result, Ok(-5));
    }

    #[test]
    fn decrby_negative_existing_integer() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(5), None)
        );
        let result = Integer::DecrBy(-10).execute(&db, "counter".into());
        assert_eq!(result, Ok(15));
    }

    #[test]
    fn decrby_underflow() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(i64::MIN), None)
        );
        let result = Integer::DecrBy(100).execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::OverflowError));
    }

    #[test]
    fn decrby_overflow() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::Integer(i64::MAX), None)
        );
        let result = Integer::DecrBy(-100).execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::OverflowError));
    }

    #[test]
    fn decrby_non_integer_value() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        db.lock().unwrap().insert(
            "counter".into(),
            Object::new(Value::String("foo".into()), None)
        );
        let result = Integer::DecrBy(10).execute(&db, "counter".into());
        assert_eq!(result, Err(ClientError::IntegerError));
    }

    #[test]
    fn decrby_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        assert_eq!(Integer::DecrBy(10).execute(&db, "counter".into()), Ok(-10));
        assert_eq!(Integer::DecrBy(10).execute(&db, "counter".into()), Ok(-20));
    }

    #[test]
    fn decrby_negative_multiple_times() {
        let db = Db::new(Mutex::new(IndexMap::new()));
        assert_eq!(Integer::DecrBy(-10).execute(&db, "counter".into()), Ok(10));
        assert_eq!(Integer::DecrBy(-10).execute(&db, "counter".into()), Ok(20));
    }
}
