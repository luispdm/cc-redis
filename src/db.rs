use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub struct Object {
    pub value: String,
    pub expiration: Option<SystemTime>,
}

impl Object {
    pub fn new(value: String, expiration: Option<SystemTime>) -> Self {
        Self { value, expiration }
    }
}

pub type Db = Arc<Mutex<HashMap<String, Object>>>;
