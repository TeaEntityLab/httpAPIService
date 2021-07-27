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

    use futures::executor::block_on;
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Client, Method, Request, Response, Server};
    use tokio::sync::Notify;
    use tokio::time::{sleep, Duration};

    use fp_rust::sync::CountDownLatch;
    // use hyper_api_service::blocking_future;
    use hyper_api_service::simple_http::SimpleHTTPDef;

    let hyper_latch = Arc::new(Notify::new());
    let started_latch = CountDownLatch::new(1);

    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    let started_latch_for_thread = started_latch.clone();
    let hyper_latch_for_thread = hyper_latch.clone();

    static TEXT: &str = "Hello, World!";

    let server = Server::bind(&addr).serve(make_service_fn(move |_| {
        let started_latch_for_thread_2 = started_latch_for_thread.clone();
        async {
            Ok::<_, hyper::Error>(service_fn(move |mut req: Request<Body>| {
                let started_latch_for_thread_3 = started_latch_for_thread_2.clone();

                async move {
                    println!("StartedB");

                    println!("Started");

                    started_latch_for_thread_3.countdown();

                    let response = Response::new(Body::from(TEXT));
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

    let mut simple_http = SimpleHTTPDef::new();
    let request = Request::builder()
        .method(Method::POST)
        .uri("http://".to_string() + &addr.to_string())
        .header("content-type", "application/json")
        .body(Body::from(r#"{"library":"hyper"}"#))
        .ok()
        .unwrap();

    println!("{:?}", request);
    let resp = simple_http.request(request).await;
    let resp_ref = resp.as_ref();
    let err = resp_ref.err();
    println!("{:?}", err);
    assert_eq!(false, resp_ref.is_err());

    started_latch.wait();
    println!("REQ",);

    hyper_latch.notify_one();

    println!("OK");
}
