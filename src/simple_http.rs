use std::collections::{HashMap, VecDeque};
use std::error::Error as StdError;
use std::result::Result as StdResult;
use std::time::Duration;

use http::method::Method;
// use futures::TryStreamExt;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::header::CONTENT_TYPE;
use hyper::{Body, Client, HeaderMap, Request, Response, Result, Uri};
use mime::MULTIPART_FORM_DATA;

#[cfg(feature = "multipart")]
use bytes::Bytes;
#[cfg(feature = "multipart")]
use formdata::FormData;
#[cfg(feature = "multipart")]
use multer;
#[cfg(feature = "multipart")]
use multer::Multipart;

// use fp_rust;

pub const DEFAULT_TIMEOUT_MILLISECOND: u64 = 30 * 1000;

pub trait Interceptor<B> {
    fn intercept(&self, request: &mut Request<B>) -> StdResult<(), Box<dyn StdError>>;
}

pub type SimpleHTTPResponse = StdResult<Result<Response<Body>>, Box<dyn StdError>>;

// SimpleHTTP SimpleHTTP inspired by Retrofits
pub struct SimpleHTTP<C, B = Body> {
    pub client: Client<C, B>,
    pub interceptors: VecDeque<Box<dyn Interceptor<B>>>,
    pub timeout_millisecond: u64,
}

impl<C, B> SimpleHTTP<C, B> {
    pub fn new_with_options(
        client: Client<C, B>,
        interceptors: VecDeque<Box<dyn Interceptor<B>>>,
        timeout_millisecond: u64,
    ) -> Self {
        SimpleHTTP {
            client,
            interceptors,
            timeout_millisecond,
        }
    }
}

impl SimpleHTTP<HttpConnector, Body> {
    /// Create a new SimpleHTTP with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new() -> SimpleHTTP<HttpConnector, Body> {
        return SimpleHTTP::new_with_options(
            Client::new(),
            VecDeque::new(),
            DEFAULT_TIMEOUT_MILLISECOND,
        );
    }
}
impl Default for SimpleHTTP<HttpConnector, Body> {
    fn default() -> SimpleHTTP<HttpConnector, Body> {
        SimpleHTTP::new()
    }
}

#[cfg(feature = "multipart")]
#[derive(Debug)]
struct FormDataParseError {
    details: String,
}
#[cfg(feature = "multipart")]
impl StdError for FormDataParseError {}
#[cfg(feature = "multipart")]
impl FormDataParseError {
    fn new(msg: impl Into<String>) -> FormDataParseError {
        FormDataParseError {
            details: msg.into(),
        }
    }
}
#[cfg(feature = "multipart")]
impl std::fmt::Display for FormDataParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.details)
    }
}

#[cfg(feature = "multipart")]
pub fn get_content_type_from_multipart_boundary(
    boundary: Vec<u8>,
) -> StdResult<String, Box<dyn StdError>> {
    Ok(MULTIPART_FORM_DATA.to_string()
        + "; boundary=\""
        + String::from_utf8(boundary)?.as_str()
        + "\"")
}
#[cfg(feature = "multipart")]
pub fn body_from_multipart(form_data: FormData) -> StdResult<(Body, Vec<u8>), Box<dyn StdError>> {
    let mut data = Vec::<u8>::new();
    let boundary = formdata::generate_boundary();
    formdata::write_formdata(&mut data, &boundary, &form_data)?;

    Ok((Body::from(data), boundary))
}
#[cfg(feature = "multipart")]
pub async fn body_to_multipart(
    headers: &HeaderMap,
    body: Body,
) -> StdResult<Multipart<'_>, Box<dyn StdError>> {
    let boundary: String;
    match headers.get(CONTENT_TYPE) {
        Some(content_type) => boundary = multer::parse_boundary(content_type.to_str()?)?,
        None => {
            return Err(Box::new(FormDataParseError::new(
                "{}: None".to_string() + CONTENT_TYPE.as_str(),
            )));
        }
    }

    Ok(Multipart::new(body, boundary))
}
#[cfg(feature = "multipart")]
pub async fn multer_multipart_to_hash_map(
    multipart: &mut Multipart<'_>,
) -> StdResult<HashMap<String, (String, String, Bytes)>, Box<dyn StdError>> {
    let mut result = HashMap::new();

    while let Some(field) = multipart.next_field().await? {
        let name = match field.name() {
            Some(s) => s.to_string(),
            None => {
                // Return error
                "".to_string()
            }
        };

        let file_name = match field.file_name() {
            Some(s) => s.to_string(),
            None => "".to_string(),
        };
        let data = if file_name.is_empty() {
            field.bytes().await?
        } else {
            Bytes::new()
        };

        result.insert(name.to_string(), (name, file_name, data));
    }

    Ok(result)
}

impl<C, B> SimpleHTTP<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn request(&self, mut request: Request<B>) -> SimpleHTTPResponse {
        for interceptor in &mut self.interceptors.iter() {
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

    pub async fn get(&self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        self.request(req).await
    }
    pub async fn head(&self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::HEAD;
        self.request(req).await
    }
    pub async fn option(&self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::OPTIONS;
        self.request(req).await
    }
    pub async fn delete(&self, uri: Uri) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::DELETE;
        self.request(req).await
    }

    pub async fn post(&self, uri: Uri, body: B) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::POST;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn put(&self, uri: Uri, body: B) -> SimpleHTTPResponse
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::PUT;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn patch(&self, uri: Uri, body: B) -> SimpleHTTPResponse
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
