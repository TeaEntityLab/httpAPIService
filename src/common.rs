use std::collections::HashMap;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/*
`PathParam` Path params for API usages
*/
pub type PathParam = HashMap<String, String>;
/*
`QueryParam` Query params for API usages
*/
pub type QueryParam = HashMap<String, String>;

#[macro_export]
macro_rules! path_param {
    ($( $key: expr => $val: expr ),*) => {{
         http_api_service::hash_map_string!(
             $( $key => $val )*
         )
    }}
}
#[macro_export]
macro_rules! query_param {
    ($( $key: expr => $val: expr ),*) => {{
         http_api_service::hash_map_string!(
             $( $key => $val )*
         )
    }}
}
#[macro_export]
macro_rules! hash_map_string {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = http_api_service::simple_api::PathParam::new();
         $( map.insert($key.into(), $val.into()); )*
         map
    }}
}

pub fn generate_id() -> String {
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    format!("{:?}{:?}", thread::current().id(), since_the_epoch)
}
