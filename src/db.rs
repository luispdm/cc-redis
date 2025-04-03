use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use indexmap::IndexMap;
use log::trace;
use rand::{rng, Rng};

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
        return 0.0
    }

    let now = SystemTime::now();
    let sample = sample_size.min(map.len());
    let mut rng = rng();

    // TODO how to prevent the retrieval of the same key multiple times?
    let keys: Vec<String> = (0..sample)
        .filter_map(|_| {
            let idx = rng.random_range(0..map.len());

            map.get_index(idx)
                .filter(|(_, o)| match o.expiration {
                    Some(exp) => now.duration_since(exp).is_ok(),
                    None => false,
                })
                .map(|(k, _)| k.to_owned())
        })
        .collect();

    for k in keys.iter() {
        map.swap_remove(k);
    }

    if !keys.is_empty() {
        trace!("removed {} expired entries", keys.len());
    }

    keys.len() as f64 / sample as f64
}
