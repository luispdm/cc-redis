use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub type Db = Arc<Mutex<HashMap<String, String>>>;
