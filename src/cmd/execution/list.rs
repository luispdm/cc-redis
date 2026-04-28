use std::collections::VecDeque;

use indexmap::map::Entry;

use crate::{
    cmd::error::ClientError,
    db::{Db, Object, Value},
};

type PushOp = Box<dyn Fn(&mut VecDeque<String>, String)>;

pub enum List {
    LPush,
    RPush,
}

impl List {
    pub fn execute(&self, db: &Db, key: String, values: Vec<String>) -> Result<usize, ClientError> {
        let mut map = db.lock().unwrap();
        let push = self.operation();

        if let Some(o) = map.get(&key) {
            if o.is_expired() {
                map.swap_remove(&key);
            }
        }

        match map.entry(key) {
            Entry::Vacant(e) => {
                let mut l = VecDeque::with_capacity(values.len());
                for v in values {
                    push(&mut l, v);
                }
                let len = l.len();
                e.insert(Object::new(Value::List(l), None));
                Ok(len)
            }
            Entry::Occupied(mut e) => match &mut e.get_mut().value {
                Value::List(l) => {
                    for v in values {
                        push(l, v);
                    }
                    Ok(l.len())
                }
                _ => Err(ClientError::WrongType),
            },
        }
    }

    pub fn operation(&self) -> PushOp {
        match self {
            List::LPush => Box::new(|l, v| l.push_front(v)),
            List::RPush => Box::new(|l, v| l.push_back(v)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use std::{
        sync::Mutex,
        time::{Duration, SystemTime},
    };

    fn empty_db() -> Db {
        Db::new(Mutex::new(IndexMap::new()))
    }

    fn assert_list(db: &Db, key: &str, expected: &[&str]) {
        let map = db.lock().unwrap();
        match &map.get(key).unwrap().value {
            Value::List(l) => {
                let got: Vec<&str> = l.iter().map(String::as_str).collect();
                assert_eq!(got, expected);
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn lpush_new_key_single() {
        let db = empty_db();
        let result = List::LPush.execute(&db, "k".into(), vec!["v".into()]);
        assert_eq!(result, Ok(1));
        assert_list(&db, "k", &["v"]);
    }

    #[test]
    fn lpush_new_key_multiple() {
        let db = empty_db();
        let result = List::LPush.execute(
            &db,
            "k".into(),
            vec!["a".into(), "b".into(), "c".into()],
        );
        assert_eq!(result, Ok(3));
        assert_list(&db, "k", &["c", "b", "a"]);
    }

    #[test]
    fn lpush_existing_list() {
        let db = empty_db();
        db.lock().unwrap().insert(
            "k".into(),
            Object::new(Value::List(VecDeque::from(vec!["x".to_string()])), None),
        );
        let result = List::LPush.execute(&db, "k".into(), vec!["a".into(), "b".into()]);
        assert_eq!(result, Ok(3));
        assert_list(&db, "k", &["b", "a", "x"]);
    }

    #[test]
    fn lpush_wrong_type_string() {
        let db = empty_db();
        db.lock().unwrap().insert(
            "k".into(),
            Object::new(Value::String("foo".into()), None),
        );
        let result = List::LPush.execute(&db, "k".into(), vec!["v".into()]);
        assert_eq!(result, Err(ClientError::WrongType));
    }

    #[test]
    fn lpush_wrong_type_integer() {
        let db = empty_db();
        db.lock()
            .unwrap()
            .insert("k".into(), Object::new(Value::Integer(5), None));
        let result = List::LPush.execute(&db, "k".into(), vec!["v".into()]);
        assert_eq!(result, Err(ClientError::WrongType));
    }

    #[test]
    fn lpush_expired_key() {
        let db = empty_db();
        db.lock().unwrap().insert(
            "k".into(),
            Object::new(
                Value::List(VecDeque::from(vec!["old".to_string()])),
                Some(SystemTime::now() - Duration::from_secs(10)),
            ),
        );
        let result = List::LPush.execute(&db, "k".into(), vec!["new".into()]);
        assert_eq!(result, Ok(1));
        assert_list(&db, "k", &["new"]);
    }

    #[test]
    fn lpush_empty_string_element() {
        let db = empty_db();
        let result = List::LPush.execute(&db, "k".into(), vec!["".into()]);
        assert_eq!(result, Ok(1));
        assert_list(&db, "k", &[""]);
    }

    #[test]
    fn rpush_new_key_single() {
        let db = empty_db();
        let result = List::RPush.execute(&db, "k".into(), vec!["v".into()]);
        assert_eq!(result, Ok(1));
        assert_list(&db, "k", &["v"]);
    }

    #[test]
    fn rpush_new_key_multiple() {
        let db = empty_db();
        let result = List::RPush.execute(
            &db,
            "k".into(),
            vec!["a".into(), "b".into(), "c".into()],
        );
        assert_eq!(result, Ok(3));
        assert_list(&db, "k", &["a", "b", "c"]);
    }

    #[test]
    fn rpush_existing_list() {
        let db = empty_db();
        db.lock().unwrap().insert(
            "k".into(),
            Object::new(Value::List(VecDeque::from(vec!["x".to_string()])), None),
        );
        let result = List::RPush.execute(&db, "k".into(), vec!["a".into(), "b".into()]);
        assert_eq!(result, Ok(3));
        assert_list(&db, "k", &["x", "a", "b"]);
    }

    #[test]
    fn rpush_wrong_type_string() {
        let db = empty_db();
        db.lock().unwrap().insert(
            "k".into(),
            Object::new(Value::String("foo".into()), None),
        );
        let result = List::RPush.execute(&db, "k".into(), vec!["v".into()]);
        assert_eq!(result, Err(ClientError::WrongType));
    }

    #[test]
    fn rpush_wrong_type_integer() {
        let db = empty_db();
        db.lock()
            .unwrap()
            .insert("k".into(), Object::new(Value::Integer(5), None));
        let result = List::RPush.execute(&db, "k".into(), vec!["v".into()]);
        assert_eq!(result, Err(ClientError::WrongType));
    }

    #[test]
    fn rpush_expired_key() {
        let db = empty_db();
        db.lock().unwrap().insert(
            "k".into(),
            Object::new(
                Value::List(VecDeque::from(vec!["old".to_string()])),
                Some(SystemTime::now() - Duration::from_secs(10)),
            ),
        );
        let result = List::RPush.execute(&db, "k".into(), vec!["new".into()]);
        assert_eq!(result, Ok(1));
        assert_list(&db, "k", &["new"]);
    }

    #[test]
    fn rpush_empty_string_element() {
        let db = empty_db();
        let result = List::RPush.execute(&db, "k".into(), vec!["".into()]);
        assert_eq!(result, Ok(1));
        assert_list(&db, "k", &[""]);
    }
}
