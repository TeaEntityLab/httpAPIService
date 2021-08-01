use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::future::Future;
use std::result::Result as StdResult;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
#[cfg(feature = "multipart")]
use formdata::FormData;
use http::method::Method;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, Client, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, HeaderMap, Request, Uri};
#[cfg(feature = "for_serde")]
use serde::{de::DeserializeOwned, Serialize};
#[cfg(feature = "for_serde")]
use serde_json;
use url::Url;

use super::simple_http;
use super::simple_http::{Interceptor, InterceptorFunc, SimpleHTTP};
// use simple_http;

/*
`PathParam` Path params for API usages
*/
pub type PathParam = HashMap<String, String>;

#[macro_export]
macro_rules! path_param {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = hyper_api_service::simple_api::PathParam::new();
         $( map.insert($key.into(), $val.into()); )*
         map
    }}
}

/*
`CommonAPI` implements `make_api_response_only()`/`make_api_no_body()`/`make_api_has_body()`,
for Retrofit-like usages.
# Arguments
* `C` - The generic type of Hyper client Connector
* `B` - The generic type of Hyper client Body
# Remarks
It's inspired by `Retrofit`.
*/
pub struct CommonAPI<C, B = Body> {
    pub simple_api: Arc<Mutex<SimpleAPI<C, B>>>,
}

impl<C, B> CommonAPI<C, B> {
    pub fn new_with_options(simple_api: Arc<Mutex<SimpleAPI<C, B>>>) -> Self {
        CommonAPI { simple_api }
    }

    pub fn set_base_url(&self, url: Url) {
        self.simple_api.lock().unwrap().base_url = url;
    }
    pub fn get_base_url_clone(&self) -> Url {
        self.simple_api.lock().unwrap().base_url.clone()
    }
    pub fn set_default_header(&self, header_map: HeaderMap) {
        self.simple_api.lock().unwrap().default_header = header_map;
    }
    pub fn get_default_header_clone(&self) -> HeaderMap {
        self.simple_api.lock().unwrap().default_header.clone()
    }
    pub fn set_client(&self, client: Client<C, B>) {
        self.simple_api.lock().unwrap().simple_http.client = client;
    }
    pub fn set_timeout_millisecond(&self, timeout_millisecond: u64) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .timeout_millisecond = timeout_millisecond;
    }
    pub fn get_timeout_millisecond(&self) -> u64 {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .timeout_millisecond
    }

    pub fn add_interceptor(&mut self, interceptor: Arc<dyn Interceptor<B>>) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .add_interceptor(interceptor);
    }
    pub fn add_interceptor_front(&mut self, interceptor: Arc<dyn Interceptor<B>>) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .add_interceptor_front(interceptor);
    }
    pub fn delete_interceptor(&mut self, interceptor: Arc<dyn Interceptor<B>>) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .delete_interceptor(interceptor);
    }
}

impl<C> CommonAPI<C, Body> {
    pub fn add_interceptor_fn(
        &mut self,
        func: impl FnMut(&mut Request<Body>) -> StdResult<(), Box<dyn StdError>> + Send + Sync + 'static,
    ) -> Arc<InterceptorFunc<Body>> {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .add_interceptor_fn(func)
    }
}

impl CommonAPI<HttpConnector, Body> {
    /// Create a new CommonAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new() -> CommonAPI<HttpConnector, Body> {
        return CommonAPI::new_with_options(Arc::new(Mutex::new(SimpleAPI::new())));
    }
}

impl<C> CommonAPI<C, Body> {
    pub fn make_api_response_only<R>(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIResponseOnly<R, C, Body> {
        APIResponseOnly {
            0: self.make_api_no_body(method, relative_url, response_deserializer, _return_type),
        }
    }
    pub fn make_api_no_body<R>(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APINoBody<R, C, Body> {
        APINoBody {
            base: CommonAPI {
                simple_api: self.simple_api.clone(),
            },
            method,
            relative_url: relative_url.into(),
            response_deserializer,
            content_type: "".to_string(),
        }
    }
    pub fn make_api_has_body<T, R>(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        request_serializer: Arc<dyn BodySerializer<T, Body>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIHasBody<T, R, C, Body> {
        APIHasBody {
            base: CommonAPI {
                simple_api: self.simple_api.clone(),
            },
            method,
            relative_url: relative_url.into(),
            content_type: content_type.into(),
            request_serializer,
            response_deserializer,
        }
    }
    #[cfg(feature = "multipart")]
    pub fn make_api_multipart<R>(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        // request_serializer: Arc<dyn BodySerializer<FormData, (String, Body)>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIMultipart<FormData, R, C, Body> {
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

impl<C> CommonAPI<C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    async fn _call_common(
        &self,
        method: Method,
        header: Option<HeaderMap>,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        path_param: impl Into<PathParam>,
        body: Body,
    ) -> StdResult<Box<Body>, Box<dyn StdError>> {
        let simple_api = self.simple_api.lock().unwrap();
        let mut req =
            simple_api.make_request(method, relative_url, content_type, path_param, body)?;

        if let Some(header) = header {
            let header_existing = req.headers_mut();
            for (k, v) in header.iter() {
                header_existing.insert(k, v.clone());
            }
        }

        let body = simple_api.simple_http.request(req).await??.into_body();

        Ok(Box::new(body))
    }

    pub async fn request(
        &self,
        method: Method,
        header: Option<HeaderMap>,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        path_param: impl Into<PathParam>,
        body: Body,
    ) -> StdResult<Box<Body>, Box<dyn StdError>> {
        self._call_common(method, header, relative_url, content_type, path_param, body)
            .await
    }
}

// APIResponseOnly API with only response options
// R: Response body Type
pub struct APIResponseOnly<R, C, B = Body>(APINoBody<R, C, B>);
impl<R, C> APIResponseOnly<R, C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(&self) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.call_with_header_additional(None).await
    }
    pub async fn call_with_header_additional(
        &self,
        header: Option<HeaderMap>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.0
            .call_with_header_additional(header, HashMap::new())
            .await
    }
}

// APINoBody API without request body options
// R: Response body Type
pub struct APINoBody<R, C, B = Body> {
    base: CommonAPI<C, B>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<R, C> APINoBody<R, C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(
        &self,
        path_param: impl Into<PathParam>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.call_with_header_additional(None, path_param).await
    }

    pub async fn call_with_header_additional(
        &self,
        header: Option<HeaderMap>,
        path_param: impl Into<PathParam>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        let mut body = self
            .base
            ._call_common(
                self.method.clone(),
                header,
                self.relative_url.clone(),
                self.content_type.clone(),
                path_param,
                Body::default(),
            )
            .await?;
        // let mut target = Box::new(target);
        // let body = Box::new(body);
        let bytes = hyper::body::to_bytes(body.as_mut()).await?;
        let target = self.response_deserializer.decode(&bytes)?;

        Ok(target)
    }
}

// APIHasBody API with request body options
// T: Request body Type
// R: Response body Type
pub struct APIHasBody<T, R, C, B = Body> {
    base: CommonAPI<C, B>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub request_serializer: Arc<dyn BodySerializer<T, B>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, C> APIHasBody<T, R, C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(
        &self,
        path_param: impl Into<PathParam>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.call_with_header_additional(None, path_param, sent_body)
            .await
    }

    pub async fn call_with_header_additional(
        &self,
        header: Option<HeaderMap>,
        path_param: impl Into<PathParam>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        // let mut sent_body = Box::new(sent_body);
        let mut body = self
            .base
            ._call_common(
                self.method.clone(),
                header,
                self.relative_url.clone(),
                self.content_type.clone(),
                path_param,
                self.request_serializer.encode(&sent_body)?,
            )
            .await?;

        // let mut target = Box::new(target);
        // let body = Box::new(body);
        let bytes = hyper::body::to_bytes(body.as_mut()).await?;
        let target = self.response_deserializer.decode(&bytes)?;

        Ok(target)
    }
}

// APIMultipart API with request body options
// T: Request body Type(multipart)
// R: Response body Type
pub struct APIMultipart<T, R, C, B = Body> {
    base: CommonAPI<C, B>,
    pub method: Method,
    pub relative_url: String,
    // pub content_type: String,
    pub request_serializer: Arc<dyn BodySerializer<T, (String, B)>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, C> APIMultipart<T, R, C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(
        &self,
        path_param: impl Into<PathParam>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.call_with_header_additional(None, path_param, sent_body)
            .await
    }

    pub async fn call_with_header_additional(
        &self,
        header: Option<HeaderMap>,
        path_param: impl Into<PathParam>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        // let mut sent_body = Box::new(sent_body);
        let (content_type_with_boundary, sent_body) = self.request_serializer.encode(&sent_body)?;
        let mut body = self
            .base
            ._call_common(
                self.method.clone(),
                header,
                self.relative_url.clone(),
                content_type_with_boundary,
                path_param,
                sent_body,
            )
            .await?;

        // let mut target = Box::new(target);
        // let body = Box::new(body);
        let bytes = hyper::body::to_bytes(body.as_mut()).await?;
        let target = self.response_deserializer.decode(&bytes)?;

        Ok(target)
    }
}

// BodySerializer Serialize the body (for put/post/patch etc)
pub trait BodySerializer<T, B = Body> {
    fn encode(&self, origin: &T) -> StdResult<B, Box<dyn StdError>>;
}
// BodyDeserializer Deserialize the body (for response)
pub trait BodyDeserializer<R> {
    fn decode(&self, bytes: &Bytes) -> StdResult<Box<R>, Box<dyn StdError>>;
}
trait Outputting: Sized {
    fn outputting<O>(self) -> Self
    where
        Self: Future<Output = O>,
    {
        self
    }
}
impl<T: Future> Outputting for T {}
// type BodyDeserializerFutureOutput<R> = StdResult<Box<R>, Box<dyn StdError>>;
// type BodyDeserializerFuture<R> = Box<dyn Future<Output = BodyDeserializerFutureOutput<R>>>;

#[derive(Debug, Clone, Copy)]
// DummyBypassSerializer Dummy bypass the body data, do nothing (for put/post/patch etc)
pub struct DummyBypassSerializer {}
impl BodySerializer<Bytes> for DummyBypassSerializer {
    fn encode(&self, origin: &Bytes) -> StdResult<Body, Box<dyn StdError>> {
        Ok(Body::from(origin.to_vec()))
    }
}
pub static DEFAULT_DUMMY_BYPASS_SERIALIZER: DummyBypassSerializer = DummyBypassSerializer {};

#[derive(Debug, Clone, Copy)]
// DummyBypassDeserializer Dummy bypass the body, do nothing (for response)
pub struct DummyBypassDeserializer {}
impl BodyDeserializer<Bytes> for DummyBypassDeserializer {
    fn decode(&self, bytes: &Bytes) -> StdResult<Box<Bytes>, Box<dyn StdError>> {
        Ok(Box::new(bytes.clone()))
    }
}
pub static DEFAULT_DUMMY_BYPASS_DESERIALIZER: DummyBypassDeserializer = DummyBypassDeserializer {};

#[cfg(feature = "multipart")]
#[derive(Debug, Clone, Copy)]
// MultipartSerializer Serialize the multipart body (for put/post/patch etc)
pub struct MultipartSerializer {}
#[cfg(feature = "multipart")]
impl BodySerializer<FormData, (String, Body)> for MultipartSerializer {
    fn encode(&self, origin: &FormData) -> StdResult<(String, Body), Box<dyn StdError>> {
        let (body, boundary) = simple_http::body_from_multipart(origin)?;
        let content_type = simple_http::get_content_type_from_multipart_boundary(boundary)?;

        Ok((content_type, body))
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
        let serialized = serde_json::to_vec(origin)?;

        Ok(Body::from(serialized))
    }
}
#[cfg(feature = "for_serde")]
pub static DEFAULT_SERDE_JSON_SERIALIZER: SerdeJsonSerializer = SerdeJsonSerializer {};

#[cfg(feature = "for_serde")]
#[derive(Debug, Clone, Copy)]
// SerdeJsonDeserializer Deserialize the body (for response)
pub struct SerdeJsonDeserializer {}
#[cfg(feature = "for_serde")]
impl<R: DeserializeOwned + 'static> BodyDeserializer<R> for SerdeJsonDeserializer {
    fn decode(&self, bytes: &Bytes) -> StdResult<Box<R>, Box<dyn StdError>> {
        let target: R = serde_json::from_slice(bytes.to_vec().as_slice())?;

        Ok(Box::new(target))
    }
}
#[cfg(feature = "for_serde")]
pub static DEFAULT_SERDE_JSON_DESERIALIZER: SerdeJsonDeserializer = SerdeJsonDeserializer {};

// SimpleAPI SimpleAPI inspired by Retrofits
pub struct SimpleAPI<C, B = Body> {
    pub simple_http: SimpleHTTP<C, B>,
    pub base_url: Url,
    pub default_header: HeaderMap,
}

impl<C, B> SimpleAPI<C, B> {
    pub fn new_with_options(simple_http: SimpleHTTP<C, B>, base_url: Url) -> Self {
        SimpleAPI {
            simple_http,
            base_url,
            default_header: HeaderMap::new(),
        }
    }
}

impl SimpleAPI<HttpConnector, Body> {
    /// Create a new SimpleAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new() -> SimpleAPI<HttpConnector, Body> {
        return SimpleAPI::new_with_options(
            SimpleHTTP::new(),
            Url::parse("http://localhost").ok().unwrap(),
        );
    }
}
impl Default for SimpleAPI<HttpConnector, Body> {
    fn default() -> SimpleAPI<HttpConnector, Body> {
        SimpleAPI::<HttpConnector, Body>::new()
    }
}

impl<C, B> SimpleAPI<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub fn make_request(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        path_param: impl Into<PathParam>,
        body: B,
    ) -> StdResult<Request<B>, Box<dyn StdError>> {
        let mut relative_url = relative_url.into();
        for (k, v) in path_param.into().into_iter() {
            relative_url = relative_url.replace(&("{".to_string() + &k + "}"), &v);
        }

        let mut req = Request::new(body);
        // Url
        match self.base_url.join(&relative_url) {
            Ok(url) => {
                *req.uri_mut() = Uri::from_str(url.as_str())?;
            }
            Err(e) => return Err(Box::new(e)),
        };
        // Method
        *req.method_mut() = method;
        // Header
        *req.headers_mut() = self.default_header.clone();
        let content_type = content_type.into();
        if !content_type.is_empty() {
            req.headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_str(&content_type)?);
        }

        Ok(req)
    }
}
impl<C> SimpleAPI<C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    #[cfg(feature = "multipart")]
    pub fn make_request_multipart(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        // content_type: String,
        path_param: impl Into<PathParam>,
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
            body,
        )
    }
}

// #[inline]
// #[derive(Debug, Clone)]
