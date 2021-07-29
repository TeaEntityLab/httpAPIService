extern crate fp_rust;
extern crate futures;
extern crate hyper;

extern crate hyper_api_service;

/*
fn connect(addr: &SocketAddr) -> std::io::Result<TcpStream> {
    let req = TcpStream::connect(addr)?;
    req.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    req.set_write_timeout(Some(Duration::from_secs(1))).unwrap();
    Ok(req)
}
*/

#[tokio::test]
async fn test_get_header() {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use hyper::header::CONTENT_TYPE;
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{body, Body, Method, Request, Response, Server};
    use tokio::sync::Notify;
    use tokio::time::{sleep, Duration};

    use fp_rust::sync::CountDownLatch;
    use hyper_api_service::simple_http::SimpleHTTP;

    let hyper_latch = Arc::new(Notify::new());
    let started_latch = CountDownLatch::new(1);

    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    let started_latch_for_thread = started_latch.clone();
    let hyper_latch_for_thread = hyper_latch.clone();

    let server = Server::bind(&addr).serve(make_service_fn(move |_| {
        let started_latch_for_thread_2 = started_latch_for_thread.clone();
        async {
            Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                let started_latch_for_thread_3 = started_latch_for_thread_2.clone();

                async move {
                    println!("StartedB");

                    println!("Started");

                    started_latch_for_thread_3.countdown();

                    let (parts, body_instance) = req.into_parts();

                    let bytes = body::to_bytes(body_instance).await?;
                    let parts_str = String::from(format!("{:?}", parts));
                    let body_str =
                        String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");

                    let response = Response::new(Body::from(body_str + parts_str.as_str()));
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
                hyper_latch_for_thread.notified().await;
            })
            .await;
    });

    sleep(Duration::from_millis(200)).await;

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

    let simple_http = SimpleHTTP::new();
    let request = Request::builder()
        .method(Method::POST)
        .uri("http://".to_string() + &addr.to_string())
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"library":"hyper"}"#))
        .ok()
        .unwrap();

    println!("{:?}", request);
    let resp = simple_http.request(request).await.ok().unwrap();
    let err = resp.as_ref().err();
    println!("{:?}", err);
    assert_eq!(false, resp.is_err());

    let mut body_instance = resp.ok().unwrap();
    let body_instance = body_instance.body_mut();
    let bytes = body::to_bytes(body_instance).await.ok().unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");

    assert_eq!("{\"library\":\"hyper\"}Parts { method: POST, uri: /, version: HTTP/1.1, headers: {\"content-type\": \"application/json\", \"host\": \"127.0.0.1:3000\", \"content-length\": \"19\"} }", body_str);

    started_latch.wait();
    println!("REQ",);

    hyper_latch.notify_one();

    println!("OK");
}

#[cfg(feature = "multipart")]
#[tokio::test]
async fn test_formdata() {
    extern crate formdata;
    extern crate multer;

    use std::iter::{FromIterator, IntoIterator};
    use std::net::SocketAddr;
    use std::sync::Arc;

    use formdata::FormData;
    use futures::executor::block_on;
    use hyper::header::CONTENT_TYPE;
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{body, Body, Method, Request, Response, Server};
    use tokio::sync::Notify;
    use tokio::time::{sleep, Duration};

    use fp_rust::sync::CountDownLatch;
    use hyper_api_service::simple_http;
    use hyper_api_service::simple_http::SimpleHTTP;

    let hyper_latch = Arc::new(Notify::new());
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

                    let multipart = simple_http::body_to_multipart(&parts.headers, body).await;

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
                hyper_latch_for_thread.notified().await;
            })
            .await;
    });

    sleep(Duration::from_millis(200)).await;

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

    let simple_http = SimpleHTTP::new();
    let form_data_origin = FormData {
        fields: vec![
            ("name".to_owned(), "Baxter".to_owned()),
            ("age".to_owned(), "1 month".to_owned()),
        ],
        files: vec![],
    };
    let (body, boundary) = simple_http::body_from_multipart(&form_data_origin)
        .ok()
        .unwrap();

    println!(
        "boundary: {:?}",
        String::from_utf8(boundary.clone()).ok().unwrap()
    );

    let request = Request::builder()
        .method(Method::POST)
        .uri("http://".to_string() + &addr.to_string())
        .header(
            CONTENT_TYPE,
            // "multipart/form-data; boundary=".to_string()
            simple_http::get_content_type_from_multipart_boundary(boundary)
                .ok()
                .unwrap(),
        )
        .body(body)
        .ok()
        .unwrap();

    println!("{:?}", request);
    let resp = simple_http.request(request).await.ok().unwrap();
    let err = resp.as_ref().err();
    println!("{:?}", err);
    assert_eq!(false, resp.is_err());

    let resp = resp.ok().unwrap();

    let body_instance = resp.into_body();
    let bytes = body::to_bytes(body_instance).await.ok().unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");

    println!("body_str: {:?}", body_str);

    assert_eq!(
        "\"age\":\"\":b\"1 month\"\n\"name\":\"\":b\"Baxter\"\n",
        body_str
    );

    started_latch.wait();
    println!("REQ",);

    hyper_latch.notify_one();

    println!("OK");
}
