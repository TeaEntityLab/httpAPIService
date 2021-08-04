use std::collections::HashMap;
use std::io;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::executor::block_on;
use futures::{channel::mpsc, SinkExt, Stream};

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

/*
Credit: https://stackoverflow.com/users/155423/shepmaster
From: https://stackoverflow.com/questions/56435409/how-do-i-stream-a-hyper-requests-body-from-a-slow-processing-side-thread-that-p
*/
pub struct WriteForStream<T>(pub mpsc::Sender<T>);

impl<T> io::Write for WriteForStream<T>
where
    T: for<'a> From<&'a [u8]> + Send + Sync + 'static,
{
    fn write(&mut self, d: &[u8]) -> io::Result<usize> {
        let len = d.len();
        block_on(self.0.send(d.into()))
            .map(|()| len)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    fn flush(&mut self) -> io::Result<()> {
        block_on(self.0.flush()).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

pub fn make_stream<T>() -> (mpsc::Sender<T>, impl Stream<Item = T>) {
    mpsc::channel(10)
}

pub fn generate_id() -> String {
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    format!("{:?}{:?}", thread::current().id(), since_the_epoch)
}
