# httpAPIService

[![tag](https://img.shields.io/github/tag/TeaEntityLab/httpAPIService.svg)](https://github.com/TeaEntityLab/httpAPIService)
[![Crates.io](https://img.shields.io/crates/d/http_api_service.svg)](https://crates.io/crates/http_api_service)
[![Travis CI Build Status](https://api.travis-ci.org/TeaEntityLab/httpAPIService.svg?branch=master)](https://travis-ci.org/TeaEntityLab/httpAPIService)
[![docs](https://img.shields.io/badge/docs-online-5023dd.svg)](https://docs.rs/http_api_service/)

[![license](https://img.shields.io/github/license/TeaEntityLab/httpAPIService.svg?style=social&label=License)](https://github.com/TeaEntityLab/httpAPIService)
[![stars](https://img.shields.io/github/stars/TeaEntityLab/httpAPIService.svg?style=social&label=Stars)](https://github.com/TeaEntityLab/httpAPIService)
[![forks](https://img.shields.io/github/forks/TeaEntityLab/httpAPIService.svg?style=social&label=Fork)](https://github.com/TeaEntityLab/httpAPIService)


A Retrofit inspired implementation for Rust.

# Why

I love Retrofit(for java), WebServiceAPI-style coding.

However it's hard to implement them in Rust, and there're few libraries to achieve parts of them.

Thus I implemented httpAPIService. I hope you would like it :)


# Features

* Retrofit-like API for WebService Restful API
  * Engine:
    * Hyper **feature: for_hyper**
    * Ureq **feature: for_ureq**
    * *`< To Be Continued I \ I /`* ...
  * Common:
    * Intercept the request: *`InterceptorFunc`* (struct) / *`Interceptor`* (trait)
    * Shared Connection Timeout: *`set_timeout_millisecond()`*
    * Shared Default Header: *`set_default_header()`*
    * Shared Client: *`set_client()`*
  * Request:
    * Serialize Struct to hyper HTTPBody: *`BodySerializer`* (trait)
  * Response:
    * Deserialize hyper HTTPBody to Struct: *`BodyDeserializer`* (trait)
* Optional:
  * *`SerdeJsonSerializer`*/*`SerdeJsonDeserializer`* **feature: for_serde**
  * *`MultipartSerializer`* **feature: multipart**

Note:
* If you want to bypass
  * Serialization, you can use *`DummyBypassSerializer`*
  * Deserialization, you can use *`DummyBypassDeserializer`*

# Dependencies

```toml
[features]
default = [
  "for_hyper", "multipart", "for_serde"
]
for_hyper = [ "hyper", "tokio", "http" ]
for_ureq = [ "ureq" ]
multipart = [ "formdata", "multer", "mime" ]
for_serde = [ "serde", "serde_json" ]
pure = []

[dependencies]

# Required
bytes = "^1.0.0"
url="^2.2.0"
futures = { version = "0.3", default-features = false, features = ["thread-pool"] }

# for_hyper
hyper = { version = "^0.14.0", optional = true, features = ["client", "http1", "http2", "stream", "tcp",] }
tokio = { version = "^1.8.0", optional = true,features = ["time", "macros",] }
http = { version = "^0.2.4", optional = true, }

# for_ureq
ureq = { version = "^2.1.0", optional = true, features = ["tls",] }

# multipart
formdata = { version = "^0.13.0", optional = true }
multer = { version = "^2.0.0", optional = true }
mime = { version = "^0.3.0", optional = true }

# for_serde
serde = { version = "^1.0", features = ["derive"], optional = true }
serde_json = { version = "^1.0", optional = true }
```

# Usage

## Setup: BaseURL/Timeout/Header, Intercept/Serializer/Deserializer

Example:

```rust

use std::sync::Arc;

use http::method::Method;
use hyper::HeaderMap;

use http_api_service::bind_hyper;
use http_api_service::path_param;
use http_api_service::simple_api;
use http_api_service::simple_api::{
    DEFAULT_SERDE_JSON_DESERIALIZER, DEFAULT_SERDE_JSON_SERIALIZER,
};

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug)]
struct Product {
    name: String,
    age: String,
}
impl Default for Product {
    fn default() -> Self {
        return Product {
            name: "".to_string(),
            age: "".to_string(),
        };
    }
}

let json_serializer = Arc::new(DEFAULT_SERDE_JSON_SERIALIZER);
let json_deserializer = Arc::new(DEFAULT_SERDE_JSON_DESERIALIZER);
let return_type_marker = &Product::default();

let common_api = bind_hyper::CommonAPI::new_for_hyper();
let mut base_service_setter = common_api.as_base_service_setter();
let base_service_shared = common_api.as_base_service_shared();

// Setup base_url
base_service_setter.set_base_url(url::Url::parse("http://localhost:3000").ok().unwrap());
// Setup timeout_millisecond
base_service_setter.set_timeout_millisecond(10 * 1000);

// Add common headers for Authentication or other usages
let mut header_map = match base_service_setter.get_default_header() {
    Some(header) => header,
    None => HeaderMap::new(),
};
header_map = bind_hyper::add_header_authentication_bearer(header_map, "MY_TOKEN")
    .ok()
    .unwrap();
base_service_setter.set_default_header(Some(header_map));

// Add interceptor for observing Requests before connections
base_service_setter.add_interceptor_fn(|req| {
    println!("REQ_CONTENT: {:?}", req);
    Ok(())
});

```

## GET/POST

Example:

```rust

// GET
let api_get_product = base_service_shared.make_api_no_body(
    base_service_shared.clone(),
    Method::GET,
    "/products/{id}",
    json_deserializer.clone(),
    return_type_marker,
);

// NOTE: You can use the HashMap<String, String> directly
// or path_param!["key1" => "val1", "key2" => "val2"])

// let path_param = [("id".into(), "3".into())]
//     .iter()
//     .cloned()
//     .collect::<simple_api::PathParam>();
let resp = api_get_product.call(Some(path_param!["id" => "3"])).await;
let model = resp.ok().unwrap(); // The deserialized model Product is here.

// POST

let api_post_product = base_service_shared.make_api_has_body(
    base_service_shared.clone(),
    Method::POST,
    "/products/{id}",
    "application/json",
    json_serializer.clone(),
    json_deserializer.clone(),
    return_type_marker,
);

let sent_body = Product {
    name: "Alien ".to_string(),
    age: "5 month".to_string(),
};
let resp = api_post_product
    .call(Some(path_param!["id" => "5"]), sent_body)
    .await;
let model = resp.ok().unwrap();

```

## Multipart

Example:

```rust

// Multipart

use formdata::FormData;

let form_data_origin = FormData {
    fields: vec![
        ("name".to_owned(), "Baxter".to_owned()),
        ("age".to_owned(), "1 month".to_owned()),
    ],
    files: vec![],
};

// POST make_api_multipart
let api_post_multipart = base_service_shared.make_api_multipart(
    base_service_shared.clone(),
    Method::POST,
    "/form",
    json_deserializer.clone(),
    return_type_marker,
);

let resp = api_post_multipart
    .call(Some(simple_api::PathParam::new()), form_data_origin)
    .await;
let model = resp.ok().unwrap();

```
