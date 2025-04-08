use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use indexmap::IndexMap;
use log::trace;
use rand::{rng, seq::index::sample};

pub struct Object {
    pub value: String,
    pub expiration: Option<SystemTime>,
}

impl Object {
    pub fn new(value: String, expiration: Option<SystemTime>) -> Self {
        Self { value, expiration }
    }
}

pub type Db = Arc<Mutex<IndexMap<String, Object>>>;

pub fn remove_expired_entries(db: &Db, sample_size: usize) -> f64 {
    let mut map = db.lock().unwrap();
    if map.is_empty() {
        return 0.0;
    }

    let now = SystemTime::now();
    let mut rng = rng();
    let sample_size = sample_size.min(map.len());

    let indexes = sample(&mut rng, map.len(), sample_size);
    let mut keys: Vec<String> = vec![];

    for i in indexes {
        if let Some((k, o)) = map.get_index(i) {
            if let Some(exp) = o.expiration {
                if now.duration_since(exp).is_ok() {
                    keys.push(k.clone());
                }
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
