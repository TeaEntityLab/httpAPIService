# hyperAPIService

A Retrofit inspired implementation for Rust.

## Dependencies

```toml
[features]
default = [
  "multipart", "for_serde"
]
multipart = [ "formdata", "multer", "mime" ]
for_serde = [ "serde", "serde_json" ]
pure = []

[dependencies]

# Required
hyper = { version = "^0.14.0", features = ["full"] }
tokio = { version = "^1.8.0", features = ["full"] }
bytes = "^1.0.0"
http = "^0.2.4"
futures="^0.3.0"
url="^2.2.0"

# multipart
formdata = { version = "^0.13.0", optional = true }
multer = { version = "^2.0.0", optional = true }
mime = { version = "^0.3.0", optional = true }

# for_serde
serde = { version = "^1.0", features = ["derive"], optional = true }
serde_json = { version = "^1.0", optional = true }
```


## Example:

```rust

let json_serializer = Arc::new(simple_api::DEFAULT_SERDE_JSON_SERIALIZER);
let json_deserializer = Arc::new(simple_api::DEFAULT_SERDE_JSON_DESERIALIZER);


let common_api = simple_api::CommonAPI::new();

common_api.set_base_url(
    url::Url::parse("http://localhost:3000")
        .ok()
        .unwrap(),
);


// GET
let api_get_product = common_api.make_api_no_body(
    Method::GET,
    "/products/{id}",
    json_deserializer.clone(),
);
let response_target = Box::new(Product {
    name: "".to_string(),
    age: "".to_string(),
    meta: None,
});
let path_param = [("id".into(), "3".into())]
    .iter()
    .cloned()
    .collect::<simple_api::PathParam>();
let resp = api_get_product.call(path_param, response_target).await;
let model = resp.ok().unwrap();

// POST

let api_post_product = common_api.make_api_has_body(
    Method::POST,
    "/products/{id}",
    "application/json",
    json_serializer.clone(),
    json_deserializer.clone(),
);

let sent_body = Box::new(Product {
    name: "Alien ".to_string(),
    age: "5 month".to_string(),
    meta: Some("123".to_string()),
});
let response_target = Box::new(Product {
    name: "".to_string(),
    age: "".to_string(),
    meta: None,
});
let path_param = [("id".into(), "5".into())]
    .iter()
    .cloned()
    .collect::<simple_api::PathParam>();

let resp = api_post_product
    .call(path_param, sent_body, response_target)
    .await;
let model = resp.ok().unwrap();

```
