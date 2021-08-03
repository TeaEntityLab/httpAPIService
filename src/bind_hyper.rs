/*!
In this module there're implementations & tests of `SimpleHTTP`.
*/

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use http::method::Method;
// use futures::TryStreamExt;
// use hyper::body::HttpBody;
use bytes::Bytes;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, Client, HeaderMap, Request, Response, Result, Uri};
use serde::Serialize;
use url::Url;

use super::common::{
    PathParam, QueryParam, DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES,
    DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES,
};
use super::simple_api::{APIMultipart, BodyDeserializer, BodySerializer, CommonAPI, SimpleAPI};
use super::simple_http::{
    ClientCommon, FormDataParseError, SimpleHTTP, SimpleHTTPResponse, DEFAULT_TIMEOUT_MILLISECOND,
};

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

#[derive(Debug, Clone, Copy)]
// DummyBypassSerializer Dummy bypass the body data, do nothing (for put/post/patch etc)
pub struct DummyBypassSerializer {}
impl BodySerializer<Bytes, Body> for DummyBypassSerializer {
    fn encode(&self, origin: &Bytes) -> StdResult<Body, Box<dyn StdError>> {
        Ok(Body::from(origin.to_vec()))
    }
}
pub static DEFAULT_DUMMY_BYPASS_SERIALIZER: DummyBypassSerializer = DummyBypassSerializer {};

#[cfg(feature = "multipart")]
#[derive(Debug, Clone, Copy)]
// MultipartSerializer Serialize the multipart body (for put/post/patch etc)
pub struct MultipartSerializer {}
#[cfg(feature = "multipart")]
impl BodySerializer<FormData, (String, Body)> for MultipartSerializer {
    fn encode(&self, origin: &FormData) -> StdResult<(String, Body), Box<dyn StdError>> {
        let (content_type, body) = DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES.encode(origin)?;

        Ok((content_type, Body::from(body)))
    }
}
#[cfg(feature = "multipart")]
pub static DEFAULT_MULTIPART_SERIALIZER: MultipartSerializer = MultipartSerializer {};

#[cfg(feature = "for_serde")]
#[derive(Debug, Clone, Copy)]
// SerdeJsonSerializer Serialize the for_serde body (for put/post/patch etc)
pub struct SerdeJsonSerializer {}
#[cfg(feature = "for_serde")]
impl<T: Serialize> BodySerializer<T, Body> for SerdeJsonSerializer {
    fn encode(&self, origin: &T) -> StdResult<Body, Box<dyn StdError>> {
        let serialized = DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES.encode(origin)?;

        Ok(Body::from(serialized))
    }
}
#[cfg(feature = "for_serde")]
pub static DEFAULT_SERDE_JSON_SERIALIZER: SerdeJsonSerializer = SerdeJsonSerializer {};

pub struct HyperClient<C, B>(Client<C, B>);
impl<C, B> ClientCommon<Client<C, B>, Request<B>, Result<Response<Body>>, B> for HyperClient<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn request(&self, req: Request<B>) -> Pin<Box<dyn Future<Output = Result<Response<Body>>>>> {
        Box::pin(self.0.request(req))
    }
}

impl SimpleHTTP<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
    /// Create a new SimpleHTTP with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new_for_hyper(
    ) -> SimpleHTTP<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
        return SimpleHTTP::new_with_options(
            Arc::new(HyperClient::<HttpConnector, Body>(Client::new())),
            VecDeque::new(),
            DEFAULT_TIMEOUT_MILLISECOND,
        );
    }
}
impl Default
    for SimpleHTTP<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body>
{
    fn default(
    ) -> SimpleHTTP<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
        SimpleHTTP::new_for_hyper()
    }
}

impl SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
    /// Create a new SimpleAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new_for_hyper(
    ) -> SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
        return SimpleAPI::new_with_options(
            SimpleHTTP::new_for_hyper(),
            Url::parse("http://localhost").ok().unwrap(),
        );
    }
}

impl Default
    for SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body>
{
    fn default(
    ) -> SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
        SimpleAPI::<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body>::new_for_hyper()
    }
}

impl<C> SimpleAPI<Client<C, Body>, Request<Body>, Result<Response<Body>>, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    #[cfg(feature = "multipart")]
    pub fn make_request_multipart(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        // content_type: String,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: FormData,
    ) -> StdResult<Request<Body>, Box<dyn StdError>> {
        let (content_type, body) = DEFAULT_MULTIPART_SERIALIZER.encode(&body)?;
        self.make_request(
            method,
            relative_url,
            // if content_type.is_empty() {
            content_type,
            // } else {
            //     content_type
            // },
            path_param,
            query_param,
            body,
        )
    }
}

impl CommonAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
    /// Create a new CommonAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new_for_hyper(
    ) -> CommonAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Body> {
        return CommonAPI::new_with_options(Arc::new(Mutex::new(SimpleAPI::new_for_hyper())));
    }
}

impl<C> CommonAPI<Client<C, Body>, Request<Body>, Result<Response<Body>>, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    pub async fn do_request_multipart(
        &self,
        method: Method,
        header: Option<HeaderMap>,
        relative_url: impl Into<String>,
        // content_type: impl Into<String>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: FormData,
    ) -> StdResult<Box<Body>, Box<dyn StdError>> {
        let (content_type, body) = DEFAULT_MULTIPART_SERIALIZER.encode(&body)?;
        self._call_common(
            method,
            header,
            relative_url,
            content_type,
            path_param,
            query_param,
            body,
        )
        .await
    }
}

impl<C> CommonAPI<Client<C, Body>, Request<Body>, Result<Response<Body>>, Body> {
    #[cfg(feature = "multipart")]
    pub fn make_api_multipart<R>(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        // request_serializer: Arc<dyn BodySerializer<FormData, (String, Body)>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIMultipart<FormData, R, Client<C, Body>, Request<Body>, Result<Response<Body>>, Body>
    {
        APIMultipart {
            base: CommonAPI {
                simple_api: self.simple_api.clone(),
            },
            method,
            relative_url: relative_url.into(),
            request_serializer: Arc::new(DEFAULT_MULTIPART_SERIALIZER),
            response_deserializer,
        }
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

impl<C, B> SimpleHTTP<Client<C, B>, Request<B>, Result<Response<B>>, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn request(
        &self,
        mut request: Request<B>,
    ) -> SimpleHTTPResponse<Result<Response<B>>> {
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

    pub async fn get(&self, uri: Uri) -> SimpleHTTPResponse<Result<Response<B>>>
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        self.request(req).await
    }
    pub async fn head(&self, uri: Uri) -> SimpleHTTPResponse<Result<Response<B>>>
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::HEAD;
        self.request(req).await
    }
    pub async fn option(&self, uri: Uri) -> SimpleHTTPResponse<Result<Response<B>>>
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::OPTIONS;
        self.request(req).await
    }
    pub async fn delete(&self, uri: Uri) -> SimpleHTTPResponse<Result<Response<B>>>
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::DELETE;
        self.request(req).await
    }

    pub async fn post(&self, uri: Uri, body: B) -> SimpleHTTPResponse<Result<Response<B>>>
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::POST;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn put(&self, uri: Uri, body: B) -> SimpleHTTPResponse<Result<Response<B>>>
    where
        B: Default,
    {
        let mut req = Request::new(B::default());
        *req.uri_mut() = uri;
        *req.method_mut() = Method::PUT;
        *req.body_mut() = body;
        self.request(req).await
    }
    pub async fn patch(&self, uri: Uri, body: B) -> SimpleHTTPResponse<Result<Response<B>>>
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
