use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

pub type Db = Arc<Mutex<HashMap<String, String>>>;
