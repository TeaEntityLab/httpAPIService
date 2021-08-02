extern crate bytes;
#[cfg(feature = "multipart")]
extern crate formdata;
extern crate futures;
extern crate http;
extern crate hyper;
#[cfg(feature = "multipart")]
extern crate mime;
#[cfg(feature = "multipart")]
extern crate multer;
#[cfg(feature = "for_serde")]
extern crate serde;
#[cfg(feature = "for_serde")]
extern crate serde_json;
extern crate tokio;
extern crate url;

pub mod common;
pub mod simple_api;
pub mod simple_http;
