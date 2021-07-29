use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::future::Future;
use std::result::Result as StdResult;
use std::str::FromStr;
use std::sync::Arc;

#[cfg(feature = "multipart")]
use formdata::FormData;
use http::method::Method;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, HeaderMap, Request, Uri};
#[cfg(feature = "for_serde")]
use serde::{de::DeserializeOwned, Serialize};
#[cfg(feature = "for_serde")]
use serde_json;
use url::Url;

use super::simple_http;
use super::simple_http::SimpleHTTP;
// use simple_http;

// PathParam Path params for API usages
type PathParam = HashMap<String, Box<dyn Debug>>;

pub struct CommonAPI<C, B = Body> {
    pub simple_api: Arc<SimpleAPI<C, B>>,
}

impl<C, B> CommonAPI<C, B> {
    pub fn new_with_options(simple_api: Arc<SimpleAPI<C, B>>) -> Self {
        CommonAPI { simple_api }
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
        return CommonAPI::new_with_options(Arc::new(SimpleAPI::new()));
    }
}

impl<C> CommonAPI<C, Body> {
    pub fn make_api_response_only<R>(
        &self,
        method: Method,
        relative_url: String,
        response_deserializer: Arc<dyn BodyDeserializer<R, Body>>,
    ) -> APIResponseOnly<R, C, Body> {
        APIResponseOnly {
            0: self.make_api_no_body(method, relative_url, response_deserializer),
        }
    }
    pub fn make_api_no_body<R>(
        &self,
        method: Method,
        relative_url: String,
        response_deserializer: Arc<dyn BodyDeserializer<R, Body>>,
    ) -> APINoBody<R, C, Body> {
        APINoBody {
            base: CommonAPI {
                simple_api: self.simple_api.clone(),
            },
            method,
            relative_url,
            response_deserializer,
            content_type: "".to_string(),
        }
    }
    pub fn make_api_has_body<T, R>(
        &self,
        method: Method,
        relative_url: String,
        content_type: String,
        request_serializer: Arc<dyn BodySerializer<T, Body>>,
        response_deserializer: Arc<dyn BodyDeserializer<R, Body>>,
    ) -> APIHasBody<T, R, C, Body> {
        APIHasBody {
            base: CommonAPI {
                simple_api: self.simple_api.clone(),
            },
            method,
            relative_url,
            content_type,
            request_serializer,
            response_deserializer,
        }
    }
    #[cfg(feature = "multipart")]
    pub fn make_api_multipart<R>(
        &self,
        method: Method,
        relative_url: String,
        // request_serializer: Arc<dyn BodySerializer<FormData, (String, Body)>>,
        response_deserializer: Arc<dyn BodyDeserializer<R, Body>>,
    ) -> APIMultipart<FormData, R, C, Body> {
        APIMultipart {
            base: CommonAPI {
                simple_api: self.simple_api.clone(),
            },
            method,
            relative_url,
            request_serializer: Arc::new(DEFAULT_MULTIPART_SERIALIZER.clone()),
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
        relative_url: String,
        content_type: String,
        path_param: impl Into<PathParam>,
        body: Body,
    ) -> StdResult<Box<Body>, Box<dyn StdError>> {
        let req =
            self.simple_api
                .make_request(method, relative_url, content_type, path_param, body)?;
        let body = self.simple_api.simple_http.request(req).await??.into_body();

        Ok(Box::new(body))
    }
}

// APIResponseOnly API with only response options
pub struct APIResponseOnly<R, C, B = Body>(APINoBody<R, C, B>);
impl<R, C> APIResponseOnly<R, C, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(&self, target: Box<R>) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.0.call(HashMap::new(), target).await
    }
}

// APINoBody API without request body options
pub struct APINoBody<R, C, B = Body> {
    base: CommonAPI<C, B>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub response_deserializer: Arc<dyn BodyDeserializer<R, B>>,
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
        target: Box<R>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        let body = self
            .base
            ._call_common(
                self.method.clone(),
                self.relative_url.clone(),
                self.content_type.clone(),
                path_param,
                Body::default(),
            )
            .await?;
        // let mut target = Box::new(target);
        // let body = Box::new(body);
        let target = self.response_deserializer.decode(body, target)?;

        Ok(target)
    }
}

// APIHasBody API with request body options
pub struct APIHasBody<T, R, C, B = Body> {
    base: CommonAPI<C, B>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub request_serializer: Arc<dyn BodySerializer<T, B>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R, B>>,
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
        sent_body: Box<T>,
        target: Box<R>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        // let mut sent_body = Box::new(sent_body);
        let body = self
            .base
            ._call_common(
                self.method.clone(),
                self.relative_url.clone(),
                self.content_type.clone(),
                path_param,
                self.request_serializer.encode(sent_body.as_ref())?,
            )
            .await?;

        // let mut target = Box::new(target);
        // let body = Box::new(body);
        let target = self.response_deserializer.decode(body, target)?;

        Ok(target)
    }
}

// APIMultipart API with request body options
pub struct APIMultipart<T, R, C, B = Body> {
    base: CommonAPI<C, B>,
    pub method: Method,
    pub relative_url: String,
    // pub content_type: String,
    pub request_serializer: Arc<dyn BodySerializer<T, (String, B)>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R, B>>,
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
        sent_body: Box<T>,
        target: Box<R>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        // let mut sent_body = Box::new(sent_body);
        let (content_type_with_boundary, sent_body) =
            self.request_serializer.encode(sent_body.as_ref())?;
        let body = self
            .base
            ._call_common(
                self.method.clone(),
                self.relative_url.clone(),
                content_type_with_boundary,
                path_param,
                sent_body,
            )
            .await?;

        // let mut target = Box::new(target);
        // let body = Box::new(body);
        let target = self.response_deserializer.decode(body, target)?;

        Ok(target)
    }
}

// BodySerializer Serialize the body (for put/post/patch etc)
pub trait BodySerializer<T, B = Body> {
    fn encode(&self, origin: &T) -> StdResult<B, Box<dyn StdError>>;
}
// BodyDeserializer Deserialize the body (for response)
pub trait BodyDeserializer<R, B = Body, A = Box<R>> {
    fn decode(&self, body: Box<B>, target: Box<R>) -> StdResult<A, Box<dyn StdError>>;
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
type BodyDeserializerFutureOutput<R> = StdResult<Box<R>, Box<dyn StdError>>;
type BodyDeserializerFuture<R> = Box<dyn Future<Output = BodyDeserializerFutureOutput<R>>>;

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
impl<R: DeserializeOwned + 'static> BodyDeserializer<R, Body, BodyDeserializerFuture<R>>
    for SerdeJsonDeserializer
{
    fn decode(
        &self,
        mut body: Box<Body>,
        mut target: Box<R>,
    ) -> StdResult<BodyDeserializerFuture<R>, Box<dyn StdError>> {
        Ok(Box::new(
            async move {
                let bytes = hyper::body::to_bytes(body.as_mut()).await?;
                (*target.as_mut()) = serde_json::from_slice(bytes.to_vec().as_slice())?;

                Ok(target)
            }
            .outputting::<BodyDeserializerFutureOutput<R>>(),
        ))
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
        relative_url: String,
        content_type: String,
        path_param: impl Into<PathParam>,
        body: B,
    ) -> StdResult<Request<B>, Box<dyn StdError>> {
        let mut req = Request::new(body);
        // Url
        match self.base_url.join(relative_url.as_str()) {
            Ok(url) => {
                let mut url = url.to_string();

                for (k, v) in path_param.into().into_iter() {
                    url = url.replace(format!("{{{}}}", k).as_str(), format!("{:?}", v).as_str())
                }
                *req.uri_mut() = Uri::from_str(url.as_str())?;
            }
            Err(e) => return Err(Box::new(e)),
        };
        // Method
        *req.method_mut() = method;
        // Header
        *req.headers_mut() = self.default_header.clone();
        if !content_type.is_empty() {
            req.headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_str(content_type.as_str())?);
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
        relative_url: String,
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
