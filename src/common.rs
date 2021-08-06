use std::collections::HashMap;
// use std::error::Error as StdError;
use std::io;
// use std::result::Result as StdResult;
// use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use futures::executor::block_on;
// use futures::task::SpawnExt;
use futures::{channel::mpsc as futureMpsc, SinkExt, Stream};

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
//*
pub struct WriteForStream(pub futureMpsc::Sender<Bytes>);

impl io::Write for WriteForStream
where
// T: for<'a> From<&'a [u8]> + Send + Sync + 'static,
{
    fn write(&mut self, d: &[u8]) -> io::Result<usize> {
        let len = d.len();
        let mut future = self.0.clone();
        let d = Bytes::from(d.to_vec());

        block_on(async {
            // tokio::spawn(async move {
            // println!("WriteForStream write content: {:?}", d.clone());
            match future.send(d).await {
                Err(e) => {
                    println!("Error: WriteForStream send -> {:?}", e);
                    let _ = future.close().await;

                    return Ok(0);
                }
                _ => {}
            };
            Ok(len)
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut future = self.0.clone();
        block_on(async {
            // tokio::spawn(async move {
            match future.flush().await {
                Err(e) => {
                    println!("Error: WriteForStream flush -> {:?}", e);
                    let _ = future.close().await;

                    return Ok(());
                }
                _ => {}
            };
            // println!("WriteForStream flush");
            Ok(())
        })
    }
}
// */
pub fn make_stream<T>() -> (futureMpsc::Sender<T>, impl Stream<Item = T>) {
    futureMpsc::channel(10)
}

pub fn generate_id() -> String {
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    format!("{:?}{:?}", thread::current().id(), since_the_epoch)
}
