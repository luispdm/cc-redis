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

// TODO move here Incr and Decr tests from requests
