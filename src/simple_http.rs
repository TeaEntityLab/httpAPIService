use std::collections::VecDeque;
use std::error::Error as StdError;
use std::result::Result as StdResult;
use std::time::Duration;

use http::method::Method;
// use futures::TryStreamExt;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::{Body, Client, Request, Response, Result, Uri};

// use fp_rust;

const DEFAULT_TIMEOUT_MILLISECOND: u64 = 30 * 1000;

pub trait Interceptor<B> {
    fn intercept(&self, request: &mut Request<B>) -> StdResult<(), Box<dyn StdError>>;
}

type SimpleHTTPResponse = StdResult<Result<Response<Body>>, Box<dyn StdError>>;

// SimpleHTTPDef SimpleHTTP inspired by Retrofits
pub struct SimpleHTTPDef<C, B = Body> {
    pub client: Client<C, B>,
    pub interceptors: VecDeque<Box<dyn Interceptor<B>>>,
    pub timeout_millisecond: u64,
}

impl<C, B> SimpleHTTPDef<C, B> {
    pub fn new_with_options(
        client: Client<C, B>,
        interceptors: VecDeque<Box<dyn Interceptor<B>>>,
        timeout_millisecond: u64,
    ) -> Self {
        SimpleHTTPDef {
            client,
            interceptors,
            timeout_millisecond,
        }
    }
}

impl SimpleHTTPDef<HttpConnector, Body> {
    /// Create a new SimpleHTTPDef with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new() -> SimpleHTTPDef<HttpConnector, Body> {
        return SimpleHTTPDef::new_with_options(
            Client::new(),
            VecDeque::new(),
            DEFAULT_TIMEOUT_MILLISECOND,
        );
    }
}
impl Default for SimpleHTTPDef<HttpConnector, Body> {
    fn default() -> SimpleHTTPDef<HttpConnector, Body> {
        SimpleHTTPDef::new()
    }
}

impl<C, B> SimpleHTTPDef<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn request(&mut self, mut request: Request<B>) -> SimpleHTTPResponse {
        for interceptor in &mut self.interceptors.iter_mut() {
            interceptor.intercept(&mut request)?;
        }

        // Implement timeout
        match tokio::time::timeout(
            Duration::from_millis(if self.timeout_millisecond > 0 {
                self.timeout_millisecond
            } else {
                DEFAULT_TIMEOUT_MILLISECOND
            }),
            self.client.request(request),
        )
        .await
        {
            Ok(result) => Ok(result),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub async fn get(&mut self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        self.request(req).await
    }
    pub async fn head(&mut self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::HEAD;
        self.request(req).await
    }
    pub async fn option(&mut self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::OPTIONS;
        self.request(req).await
    }
    pub async fn delete(&mut self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::DELETE;
        self.request(req).await
    }

    pub async fn post(&mut self, uri: Uri, body: B) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::POST;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn put(&mut self, uri: Uri, body: B) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::PUT;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn patch(&mut self, uri: Uri, body: B) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::PATCH;
        *req.body_mut() = body;
        self.request(req).await
    }
}

// #[inline]
// #[derive(Debug, Clone)]
