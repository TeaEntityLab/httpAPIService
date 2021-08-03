extern crate futures;

extern crate hyper_api_service;

/*
#[cfg(feature = "default")]
#[tokio::test]
async fn test_simple_api_for_readme_md() {
    extern crate hyper;

    use std::sync::Arc;

    use http::method::Method;
    use hyper::HeaderMap;

    use hyper_api_service::bind_hyper;
    use hyper_api_service::path_param;
    use hyper_api_service::simple_api;
    use hyper_api_service::simple_api::{
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
}
// */
#[cfg(feature = "default")]
#[tokio::test]
async fn test_simple_api_common() {
    extern crate fp_rust;
    extern crate formdata;
    extern crate multer;

    use std::net::SocketAddr;
    use std::sync::Arc;

    use hyper::service::{make_service_fn, service_fn};
    use hyper::{body, Body, HeaderMap, Method, Request, Response, Server};
    use serde::{Deserialize, Serialize};

    use fp_rust::sync::CountDownLatch;
    use hyper_api_service::bind_hyper;
    use hyper_api_service::bind_hyper::add_header_authentication_bearer;
    use hyper_api_service::simple_api;
    use hyper_api_service::simple_api::{
        DEFAULT_SERDE_JSON_DESERIALIZER, DEFAULT_SERDE_JSON_SERIALIZER,
    };
    use hyper_api_service::{path_param, query_param};

    #[derive(Serialize, Deserialize, Debug)]
    struct Product {
        name: String,
        age: String,
        meta: Option<String>,
    }
    impl Default for Product {
        fn default() -> Self {
            return Product {
                name: "".to_string(),
                age: "".to_string(),
                meta: None,
            };
        }
    }

    let hyper_latch = CountDownLatch::new(1);
    let started_latch = CountDownLatch::new(1);

    let addr: SocketAddr = ([127, 0, 0, 1], 3400).into();

    let started_latch_for_thread = started_latch.clone();
    let hyper_latch_for_thread = hyper_latch.clone();

    let server = Server::bind(&addr).serve(make_service_fn(move |_| {
        let started_latch_for_thread_2 = started_latch_for_thread.clone();
        async {
            Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                let started_latch_for_thread_3 = started_latch_for_thread_2.clone();

                async move {
                    println!("Started");

                    started_latch_for_thread_3.countdown();

                    let (parts, body_instance) = req.into_parts();

                    let method = &parts.method;
                    if method.eq(&Method::PUT)
                        || method.eq(&Method::POST)
                        || method.eq(&Method::PATCH)
                    {
                        let bytes = body::to_bytes(body_instance).await?;
                        let body_str = String::from_utf8(bytes.to_vec())
                            .expect("response was not valid utf-8");

                        let mut deserialized: Product =
                            serde_json::from_str(&body_str.as_str()).unwrap();
                        deserialized.name = deserialized.name + " modified";
                        deserialized.age = "3 years".into();
                        deserialized.meta = Some(format!("{:?}", parts));
                        let serialized = serde_json::to_string(&deserialized).unwrap();

                        let response = Response::new(Body::from(serialized));
                        Ok::<Response<Body>, hyper::Error>(response)
                    } else {
                        let model = Product {
                            name: "Baxter from server".to_string(),
                            age: "1 month from server".to_string(),
                            meta: Some(format!("{:?}", parts)),
                        };
                        let serialized = serde_json::to_string(&model).unwrap();

                        let response = Response::new(Body::from(serialized));
                        Ok::<Response<Body>, hyper::Error>(response)
                    }
                }
            }))
        }
    }));

    println!("Started C");

    tokio::spawn(async {
        // hyper_latch_for_thread.countdown();
        let _ = server
            .with_graceful_shutdown(async move {
                hyper_latch_for_thread.await;
                println!("!!!!!!!!!");
            })
            .await;
    });

    // sleep(Duration::from_millis(20)).await;

    /*
    let mut req = connect(&addr).unwrap();
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Content-Length: 19\r\n\
        \r\n\
        I'm a good request.\r\n\
    ",
    )
    .unwrap();
    req.read(&mut [0; 256]).unwrap();
    */

    let common_api = bind_hyper::CommonAPI::new_for_hyper();
    let mut base_service_setter = common_api.as_base_service_setter();
    let base_service_shared = common_api.as_base_service_shared();
    base_service_setter.set_base_url(
        url::Url::parse(&("http://".to_string() + addr.to_string().as_str()))
            .ok()
            .unwrap(),
    );
    // Setup timeout_millisecond
    base_service_setter.set_timeout_millisecond(10 * 1000);

    let mut header_map = if let Some(header) = base_service_setter.get_default_header() {
        header
    } else {
        HeaderMap::new()
    };
    header_map = add_header_authentication_bearer(header_map, "MY_TOKEN")
        .ok()
        .unwrap();
    base_service_setter.set_default_header(Some(header_map));

    base_service_setter.add_interceptor_fn(|req| {
        println!("REQ_CONTENT: {:?}", req);
        Ok(())
    });

    let json_serializer = Arc::new(DEFAULT_SERDE_JSON_SERIALIZER);
    let json_deserializer = Arc::new(DEFAULT_SERDE_JSON_DESERIALIZER);
    let return_type_marker = &Product::default();

    // GET make_api_response_only
    {
        let api_get_products = base_service_setter.make_api_response_only(
            base_service_shared.clone(),
            Method::GET,
            "/products",
            json_deserializer.clone(),
            return_type_marker,
        );

        let resp = api_get_products.call().await;
        let err = resp.as_ref().err();
        println!("{:?}", err);
        assert_eq!(false, resp.is_err());
        let model = resp.ok().unwrap();
        let serialized = serde_json::to_string(model.as_ref()).unwrap();
        println!("serialized: {:?}", serialized);
        assert_eq!(
            "{\"name\":\"Baxter from server\",\"age\":\"1 month from server\",\"meta\":\"Parts { method: GET, uri: /products, version: HTTP/1.1, headers: {\\\"authorization\\\": \\\"Bearer MY_TOKEN\\\", \\\"host\\\": \\\"127.0.0.1:3400\\\"} }\"}",
            serialized
        );
    }
    // DELETE make_api_no_body
    {
        let api_delete_product = base_service_setter.make_api_no_body(
            base_service_shared.clone(),
            Method::DELETE,
            "/products/{id}",
            json_deserializer.clone(),
            return_type_marker,
        );

        let path_param = [("id".into(), "3".into())]
            .iter()
            .cloned()
            .collect::<simple_api::PathParam>();

        let resp = api_delete_product
            .call_with_options(None, Some(path_param), Some(query_param!("soft" => "true")))
            .await;
        let err = resp.as_ref().err();
        println!("{:?}", err);
        assert_eq!(false, resp.is_err());
        let model = resp.ok().unwrap();
        let serialized = serde_json::to_string(model.as_ref()).unwrap();
        println!("serialized: {:?}", serialized);
        assert_eq!(
            "{\"name\":\"Baxter from server\",\"age\":\"1 month from server\",\"meta\":\"Parts { method: DELETE, uri: /products/3?soft=true, version: HTTP/1.1, headers: {\\\"authorization\\\": \\\"Bearer MY_TOKEN\\\", \\\"host\\\": \\\"127.0.0.1:3400\\\"} }\"}",
            serialized
        );
    }
    // PUT make_api_has_body
    {
        let api_put_product = base_service_setter.make_api_has_body(
            base_service_shared.clone(),
            Method::PUT,
            "/products/{id}",
            "application/json",
            json_serializer.clone(),
            json_deserializer.clone(),
            return_type_marker,
        );

        let sent_body = Product {
            name: "Alien ".to_string(),
            age: "5 month".to_string(),
            meta: Some("123".to_string()),
        };

        let resp = api_put_product
            .call(Some(path_param!["id" => "5"]), sent_body)
            .await;
        let err = resp.as_ref().err();
        println!("{:?}", err);
        assert_eq!(false, resp.is_err());
        let model = resp.ok().unwrap();
        let serialized = serde_json::to_string(model.as_ref()).unwrap();
        println!("serialized: {:?}", serialized);
        assert_eq!(
            "{\"name\":\"Alien  modified\",\"age\":\"3 years\",\"meta\":\"Parts { method: PUT, uri: /products/5, version: HTTP/1.1, headers: {\\\"authorization\\\": \\\"Bearer MY_TOKEN\\\", \\\"content-type\\\": \\\"application/json\\\", \\\"host\\\": \\\"127.0.0.1:3400\\\", \\\"content-length\\\": \\\"46\\\"} }\"}",
            serialized
        );
    }

    started_latch.wait();
    println!("REQ",);

    hyper_latch.countdown();

    println!("OK");
}

#[cfg(feature = "default")]
#[tokio::test]
async fn test_simple_api_formdata() {
    extern crate formdata;
    extern crate multer;

    use std::iter::{FromIterator, IntoIterator};
    use std::net::SocketAddr;
    use std::sync::Arc;

    use formdata::FormData;
    use futures::executor::block_on;
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Method, Request, Response, Server};

    use fp_rust::sync::CountDownLatch;
    use hyper_api_service::bind_hyper;
    use hyper_api_service::bind_hyper::body_to_multipart;
    use hyper_api_service::simple_api;
    use hyper_api_service::simple_http;

    let hyper_latch = CountDownLatch::new(1);
    let started_latch = CountDownLatch::new(1);

    let addr: SocketAddr = ([127, 0, 0, 1], 3300).into();

    let started_latch_for_thread = started_latch.clone();
    let hyper_latch_for_thread = hyper_latch.clone();

    let server = Server::bind(&addr).serve(make_service_fn(move |_| {
        let started_latch_for_thread_2 = started_latch_for_thread.clone();
        async {
            Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                let started_latch_for_thread_3 = started_latch_for_thread_2.clone();

                async move {
                    println!("Started");

                    started_latch_for_thread_3.countdown();

                    // let (_, body_instance) = req.into_parts();
                    // let bytes = body::to_bytes(body_instance).await?;
                    // let body_str =
                    //     String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");

                    let (parts, body) = req.into_parts();

                    let multipart = body_to_multipart(&parts.headers, body).await;

                    println!("Error: {:?}", multipart.as_ref().err());

                    let mut multipart = multipart.ok().unwrap();

                    let hash_map =
                        block_on(simple_http::multer_multipart_to_hash_map(&mut multipart));
                    let hash_map = hash_map.ok().unwrap();

                    let mut body_str = String::new();
                    let mut keys = Vec::from_iter(hash_map.keys().into_iter());
                    keys.sort();
                    for k in keys.into_iter() {
                        let item = hash_map.get(k).expect("hash_map unwrap failed");
                        body_str =
                            body_str + format!("{:?}:{:?}:{:?}\n", item.0, item.1, item.2).as_str();
                    }

                    let response = Response::new(Body::from(body_str));
                    Ok::<Response<Body>, hyper::Error>(response)
                }
            }))
        }
    }));

    println!("Started C");

    tokio::spawn(async {
        // hyper_latch_for_thread.countdown();
        let _ = server
            .with_graceful_shutdown(async move {
                hyper_latch_for_thread.await;
            })
            .await;
    });

    // sleep(Duration::from_millis(20)).await;

    /*
    let mut req = connect(&addr).unwrap();
    req.write_all(
        b"\
        GET / HTTP/1.1\r\n\
        Host: example.domain\r\n\
        Content-Length: 19\r\n\
        \r\n\
        I'm a good request.\r\n\
    ",
    )
    .unwrap();
    req.read(&mut [0; 256]).unwrap();
    */

    let form_data_origin = FormData {
        fields: vec![
            ("name".to_owned(), "Baxter".to_owned()),
            ("age".to_owned(), "1 month".to_owned()),
        ],
        files: vec![],
    };

    let common_api = bind_hyper::CommonAPI::new_for_hyper();
    let base_service_setter = common_api.as_base_service_setter();
    let base_service_shared = common_api.as_base_service_shared();

    base_service_setter.set_base_url(
        url::Url::parse(&("http://".to_string() + addr.to_string().as_str()))
            .ok()
            .unwrap(),
    );

    // POST make_api_multipart
    let api_post_multipart = base_service_setter.make_api_multipart(
        base_service_shared.clone(),
        Method::POST,
        "/form",
        Arc::new(simple_api::DEFAULT_DUMMY_BYPASS_DESERIALIZER),
        &bytes::Bytes::new(),
    );

    let resp = api_post_multipart
        .call(Some(simple_api::PathParam::new()), form_data_origin)
        .await;
    let err = resp.as_ref().err();
    println!("{:?}", err);
    assert_eq!(false, resp.is_err());

    let resp = resp.ok().unwrap();

    // let body_instance = resp.into_body();
    // let bytes = body::to_bytes(body_instance).await.ok().unwrap();
    // let body_str = String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");

    println!("resp: {:?}", resp);

    assert_eq!(
        "\"age\":\"\":b\"1 month\"\n\"name\":\"\":b\"Baxter\"\n",
        String::from_utf8(resp.to_vec()).ok().unwrap().as_str()
    );

    started_latch.wait();
    println!("REQ",);

    hyper_latch.countdown();

    println!("OK");
}
