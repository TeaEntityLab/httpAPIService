extern crate bytes;
#[cfg(feature = "multipart")]
extern crate formdata;
extern crate futures;
extern crate http;
#[cfg(feature = "for_hyper")]
extern crate hyper;
#[cfg(feature = "multipart")]
extern crate mime;
#[cfg(feature = "multipart")]
extern crate multer;
#[cfg(feature = "for_serde")]
extern crate serde;
#[cfg(feature = "for_serde")]
extern crate serde_json;
#[cfg(feature = "for_hyper")]
extern crate tokio;
extern crate url;

#[cfg(feature = "for_hyper")]
pub mod bind_hyper;

pub mod common;
pub mod simple_api;
pub mod simple_http;
