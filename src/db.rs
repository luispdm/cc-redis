use std::{
    fmt::Display,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use indexmap::IndexMap;
use log::trace;
use rand::{rng, seq::index::sample};

#[derive(Debug, PartialEq)]
pub enum Value {
    Integer(i64),
    String(String),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(i) => write!(f, "{}", i),
            Value::String(s) => write!(f, "{}", s),
        }
    }
}

pub struct Object {
    pub value: Value,
    pub expiration: Option<SystemTime>,
}

impl Object {
    pub fn new(value: Value, expiration: Option<SystemTime>) -> Self {
        Self { value, expiration }
    }
}

pub type Db = Arc<Mutex<IndexMap<String, Object>>>;

pub fn remove_expired_entries(db: &Db, sample_size: usize) -> f64 {
    let mut map = db.lock().unwrap();
    if map.is_empty() {
        return 0.0;
    }

    let mut rng = rng();
    let sample_size = sample_size.min(map.len());

    let indexes = sample(&mut rng, map.len(), sample_size);
    let mut keys: Vec<String> = vec![];

    for i in indexes {
        if let Some((k, o)) = map.get_index(i) {
            if let ExpirationStatus::Expired = ExpirationStatus::get(Some(o)) {
                keys.push(k.clone());
            }
        }
    }

    if !keys.is_empty() {
        for k in keys.iter() {
            map.swap_remove(k);
        }
        trace!("removed {} expired entries", keys.len());
    }

    keys.len() as f64 / sample_size as f64
}

pub enum ExpirationStatus<'a> {
    NotExist,
    Expired,
    NotExpired(&'a Object),
}

impl<'a> ExpirationStatus<'a> {
    pub fn get(object: Option<&'a Object>) -> Self {
        let now = SystemTime::now();

        match object {
            None => Self::NotExist,
            Some(obj) => match obj.expiration {
                None => Self::NotExpired(obj),
                Some(exp) => match exp.duration_since(now) {
                    Err(_) => Self::Expired,
                    Ok(_) => Self::NotExpired(obj),
                },
            },
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use uuid::Uuid;

    use super::*;

    fn create_object(value: &str, expires_in_s: Option<i64>) -> Object {
        let expiration = expires_in_s.map(|s| {
            if s >= 0 {
                SystemTime::now()
                    .checked_add(Duration::from_secs(s as u64))
                    .unwrap()
            } else {
                SystemTime::now()
                    .checked_sub(Duration::from_secs((-s) as u64))
                    .unwrap()
            }
        });

        Object {
            value: Value::String(value.to_string()),
            expiration,
        }
    }

    fn create_test_db(entries: Vec<(String, Object)>) -> Db {
        let mut map = IndexMap::new();
        for (key, obj) in entries {
            map.insert(key, obj);
        }
        Arc::new(Mutex::new(map))
    }

    #[test]
    fn expiration_status_not_exist() {
        let db = create_test_db(vec![]);
        let map = db.lock().unwrap();
        let status = ExpirationStatus::get(map.get("key"));
        assert!(matches!(status, ExpirationStatus::NotExist));
    }

    #[test]
    fn expiration_status_no_expiration() {
        let entries = vec![("key".to_string(), create_object("value", None))];
        let db = create_test_db(entries);
        let map = db.lock().unwrap();

        let status = ExpirationStatus::get(map.get("key"));
        match status {
            ExpirationStatus::NotExpired(returned_obj) => {
                assert_eq!(returned_obj.value, Value::String("value".to_string()));
                assert!(returned_obj.expiration.is_none());
            }
            _ => panic!("Expected NotExpired variant"),
        }
    }

    #[test]
    fn expiration_status_expired() {
        let entries = vec![("key".to_string(), create_object("value", Some(-1)))];
        let db = create_test_db(entries);
        let map = db.lock().unwrap();

        let status = ExpirationStatus::get(map.get("key"));
        assert!(matches!(status, ExpirationStatus::Expired));
    }

    #[test]
    fn expiration_status_not_expired() {
        let obj = create_object("value", Some(3600));
        let expiration_time = obj.expiration.unwrap();
        let entries = vec![("key".to_string(), obj)];

        let db = create_test_db(entries);
        let map = db.lock().unwrap();
        let status = ExpirationStatus::get(map.get("key"));

        match status {
            ExpirationStatus::NotExpired(returned_obj) => {
                assert_eq!(returned_obj.value, Value::String("value".to_string()));
                assert_eq!(returned_obj.expiration, Some(expiration_time));
            }
            _ => panic!("Expected NotExpired variant"),
        }
    }

    #[test]
    fn empty_map() {
        let db = create_test_db(vec![]);
        let result = remove_expired_entries(&db, 10);
        assert_eq!(result, 0.0);
        assert_eq!(db.lock().unwrap().len(), 0);
    }

    #[test]
    fn not_yet_expired_entries() {
        let entries = vec![
            (Uuid::new_v4().to_string(), create_object("val1", Some(100))),
            (Uuid::new_v4().to_string(), create_object("val2", Some(100))),
            (Uuid::new_v4().to_string(), create_object("val3", Some(100))),
        ];
        let original_len = entries.len();
        let db = create_test_db(entries);

        let result = remove_expired_entries(&db, original_len);
        assert_eq!(result, 0.0);
        assert_eq!(db.lock().unwrap().len(), original_len);
    }

    #[test]
    fn no_expiration_entries() {
        let key1 = Uuid::new_v4().to_string();

        let entries = vec![(key1.clone(), create_object("val1", None))];
        let db = create_test_db(entries);

        let result = remove_expired_entries(&db, 1);
        assert_eq!(result, 0.0);

        let locked_db = db.lock().unwrap();
        assert_eq!(locked_db.len(), 1);
        assert!(locked_db.contains_key(&key1));
    }

    #[test]
    fn all_entries_expired() {
        let entries = vec![
            (Uuid::new_v4().to_string(), create_object("val1", Some(-1))),
            (Uuid::new_v4().to_string(), create_object("val2", Some(-2))),
            (Uuid::new_v4().to_string(), create_object("val3", Some(-3))),
        ];
        let original_len = entries.len();
        let db = create_test_db(entries);

        let result = remove_expired_entries(&db, original_len);
        assert_eq!(result, 1.0);
        assert_eq!(db.lock().unwrap().len(), 0);
    }

    #[test]
    fn some_entries_expired() {
        let expired_key1 = Uuid::new_v4().to_string();
        let expired_key2 = Uuid::new_v4().to_string();
        let valid_key = Uuid::new_v4().to_string();

        let entries = vec![
            (expired_key1.clone(), create_object("expired1", Some(-1))),
            (valid_key.clone(), create_object("valid", Some(100))),
            (expired_key2.clone(), create_object("expired2", Some(-2))),
        ];
        let db = create_test_db(entries);

        // 2 out of 3 expired
        let result = remove_expired_entries(&db, 3);
        assert!(result > 0.65 && result < 0.67);

        let locked_db = db.lock().unwrap();
        assert_eq!(locked_db.len(), 1);
        assert!(locked_db.contains_key(&valid_key));
    }

    #[test]
    fn sample_size_smaller_than_map() {
        let mut entries = vec![];
        let mut expired_keys = vec![];

        for i in 0..5 {
            let key = Uuid::new_v4().to_string();
            entries.push((
                key.clone(),
                create_object(&format!("expired{}", i), Some(-1)),
            ));
            expired_keys.push(key);
        }

        for i in 0..5 {
            entries.push((
                Uuid::new_v4().to_string(),
                create_object(&format!("valid{}", i), Some(100)),
            ));
        }

        let db = create_test_db(entries);

        let ratio = remove_expired_entries(&db, 3);
        assert!((0.0..=1.0).contains(&ratio));

        // at most 3 entries should have been removed
        let locked_db = db.lock().unwrap();
        assert!(10 - locked_db.len() <= 3);
    }

    #[test]
    fn just_expired_entries() {
        let just_expired_key = Uuid::new_v4().to_string();

        let entries = vec![(
            just_expired_key.clone(),
            create_object("just_expired", Some(0)),
        )];
        let db = create_test_db(entries);

        std::thread::sleep(Duration::from_millis(1));

        let result = remove_expired_entries(&db, 2);
        assert_eq!(result, 1.0);

        let locked_db = db.lock().unwrap();
        assert_eq!(locked_db.len(), 0);
    }
}
