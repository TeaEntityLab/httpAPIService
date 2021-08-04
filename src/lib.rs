// Crates

extern crate bytes;
extern crate futures;
extern crate http;
extern crate url;

#[cfg(feature = "for_hyper")]
extern crate hyper;
#[cfg(feature = "for_hyper")]
extern crate tokio;

#[cfg(feature = "multipart")]
extern crate formdata;
#[cfg(feature = "multipart")]
extern crate mime;
#[cfg(feature = "multipart")]
extern crate multer;

#[cfg(feature = "for_serde")]
extern crate serde;
#[cfg(feature = "for_serde")]
extern crate serde_json;

// MODs

pub mod common;
pub mod simple_api;
pub mod simple_http;

#[cfg(feature = "for_hyper")]
pub mod bind_hyper;

#[cfg(feature = "for_ureq")]
pub mod bind_ureq;
