/*!
In this module there're implementations & tests of `SimpleHTTP`.
*/

use std::collections::{HashMap, VecDeque};
use std::error::Error as StdError;
use std::result::Result as StdResult;
use std::sync::Arc;
use std::time::Duration;

use http::method::Method;
// use futures::TryStreamExt;
// use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, Client, HeaderMap, Request, Response, Result, Uri};

pub use super::common::{Interceptor, InterceptorFunc};
use bytes::Bytes;

#[cfg(feature = "multipart")]
pub use super::common::{
    data_and_boundary_from_multipart, generate_id, get_content_type_from_multipart_boundary,
};
#[cfg(feature = "multipart")]
use formdata::FormData;
#[cfg(feature = "multipart")]
use multer;
#[cfg(feature = "multipart")]
use multer::Multipart;

pub const DEFAULT_TIMEOUT_MILLISECOND: u64 = 30 * 1000;

pub type SimpleHTTPResponse<B> = StdResult<Result<Response<B>>, Box<dyn StdError>>;

/* SimpleHTTP SimpleHTTP inspired by Retrofits
*/
pub struct SimpleHTTP<C, B> {
    pub client: Client<C, B>,
    pub interceptors: VecDeque<Arc<dyn Interceptor<Request<B>>>>,
    pub timeout_millisecond: u64,
}

impl<C, B> SimpleHTTP<C, B> {
    pub fn new_with_options(
        client: Client<C, B>,
        interceptors: VecDeque<Arc<dyn Interceptor<Request<B>>>>,
        timeout_millisecond: u64,
    ) -> Self {
        SimpleHTTP {
            client,
            interceptors,
            timeout_millisecond,
        }
    }

    pub fn add_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Request<B>>>) {
        self.interceptors.push_back(interceptor);
    }
    pub fn add_interceptor_front(&mut self, interceptor: Arc<dyn Interceptor<Request<B>>>) {
        self.interceptors.push_front(interceptor);
    }
    pub fn delete_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Request<B>>>) {
        let id;
        {
            id = interceptor.get_id();
        }

        for (index, obs) in self.interceptors.clone().iter().enumerate() {
            if obs.get_id() == id {
                // println!("delete_interceptor({});", interceptor);
                self.interceptors.remove(index);
                return;
            }
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

pub fn add_header_authentication(
    mut header_map: HeaderMap,
    token: impl Into<String>,
) -> StdResult<HeaderMap, Box<dyn StdError>> {
    let str = token.into();
    header_map.insert("Authorization", HeaderValue::from_str(&str)?);

    Ok(header_map)
}

pub fn add_header_authentication_bearer(
    header_map: HeaderMap,
    token: impl Into<String>,
) -> StdResult<HeaderMap, Box<dyn StdError>> {
    return add_header_authentication(header_map, "Bearer ".to_string() + &token.into());
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
pub fn body_from_multipart(form_data: &FormData) -> StdResult<(Body, Vec<u8>), Box<dyn StdError>> {
    let (data, boundary) = data_and_boundary_from_multipart(form_data)?;

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

impl<C> SimpleHTTP<C, Body> {
    pub fn add_interceptor_fn(
        &mut self,
        func: impl FnMut(&mut Request<Body>) -> StdResult<(), Box<dyn StdError>> + Send + Sync + 'static,
    ) -> Arc<InterceptorFunc<Request<Body>>> {
        let interceptor = Arc::new(InterceptorFunc::new(func));
        self.add_interceptor(interceptor.clone());

        interceptor
    }
}

impl<C> SimpleHTTP<C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn request(&self, mut request: Request<Body>) -> SimpleHTTPResponse<Body> {
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

    pub async fn get(&self, uri: Uri) -> SimpleHTTPResponse<Body>
    where
        Body: Default,
    {
        let mut req = Request::new(Body::default());
        *req.uri_mut() = uri;
        self.request(req).await
    }
    pub async fn head(&self, uri: Uri) -> SimpleHTTPResponse<Body>
    where
        Body: Default,
    {
        let mut req = Request::new(Body::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::HEAD;
        self.request(req).await
    }
    pub async fn option(&self, uri: Uri) -> SimpleHTTPResponse<Body>
    where
        Body: Default,
    {
        let mut req = Request::new(Body::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::OPTIONS;
        self.request(req).await
    }
    pub async fn delete(&self, uri: Uri) -> SimpleHTTPResponse<Body>
    where
        Body: Default,
    {
        let mut req = Request::new(Body::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::DELETE;
        self.request(req).await
    }

    pub async fn post(&self, uri: Uri, body: Body) -> SimpleHTTPResponse<Body>
    where
        Body: Default,
    {
        let mut req = Request::new(Body::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::POST;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn put(&self, uri: Uri, body: Body) -> SimpleHTTPResponse<Body>
    where
        Body: Default,
    {
        let mut req = Request::new(Body::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::PUT;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn patch(&self, uri: Uri, body: Body) -> SimpleHTTPResponse<Body>
    where
        Body: Default,
    {
        let mut req = Request::new(Body::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::PATCH;
        *req.body_mut() = body;
        self.request(req).await
    }
}

// #[inline]
// #[derive(Debug, Clone)]
