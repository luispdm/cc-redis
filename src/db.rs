use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

pub type Db = Arc<RwLock<HashMap<String, String>>>;
