#[cfg(feature = "multipart")]
extern crate formdata;
extern crate futures;
extern crate http;
extern crate hyper;
#[cfg(feature = "multipart")]
extern crate mime;
#[cfg(feature = "multipart")]
extern crate multer;
extern crate tokio;
extern crate url;

extern crate fp_rust;

pub mod simple_api;
pub mod simple_http;
