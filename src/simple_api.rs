/*!
In this module there're implementations & tests of `SimpleAPI`.
*/

use std::error::Error as StdError;
use std::fmt::Debug;
use std::future::Future;
use std::result::Result as StdResult;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::method::Method;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, Client, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, HeaderMap, Request, Response, Result, Uri};
use url::Url;

pub use super::common::{
    BodyDeserializer, BodySerializer, PathParam, QueryParam, DEFAULT_DUMMY_BYPASS_DESERIALIZER,
    DEFAULT_DUMMY_BYPASS_SERIALIZER_FOR_BYTES,
};
use super::simple_http::{ClientCommon, Interceptor, InterceptorFunc, SimpleHTTP};

#[cfg(feature = "for_serde")]
pub use super::common::DEFAULT_SERDE_JSON_DESERIALIZER;
#[cfg(feature = "for_serde")]
use super::common::DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES;
#[cfg(feature = "for_serde")]
use serde::Serialize;

#[cfg(feature = "multipart")]
use super::common::DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES;
#[cfg(feature = "multipart")]
use formdata::FormData;

/*
`CommonAPI` implements `make_api_response_only()`/`make_api_no_body()`/`make_api_has_body()`,
for Retrofit-like usages.
# Arguments
* `C` - The generic type of Hyper client Connector
* `B` - The generic type of Hyper client Body
# Remarks
It's inspired by `Retrofit`.
*/
pub struct CommonAPI<Client, Req, Res, Method, B> {
    pub simple_api: Arc<Mutex<SimpleAPI<Client, Req, Res, Method, B>>>,
}

impl<Client, Req, Res, Method, B> CommonAPI<Client, Req, Res, Method, B> {
    pub fn new_with_options(
        simple_api: Arc<Mutex<SimpleAPI<Client, Req, Res, Method, B>>>,
    ) -> Self {
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
    pub fn set_client(&self, client: Arc<dyn ClientCommon<Client, Req, Res, Method, B>>) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .set_client(client);
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

    pub fn add_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .add_interceptor(interceptor);
    }
    pub fn add_interceptor_front(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .add_interceptor_front(interceptor);
    }
    pub fn delete_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .delete_interceptor(interceptor);
    }
}

impl<Client, Req, Res, Method, B> CommonAPI<Client, Req, Res, Method, B>
where
    Req: 'static,
{
    pub fn add_interceptor_fn(
        &mut self,
        func: impl FnMut(&mut Req) -> StdResult<(), Box<dyn StdError>> + Send + Sync + 'static,
    ) -> Arc<InterceptorFunc<Req>> {
        self.simple_api
            .lock()
            .unwrap()
            .simple_http
            .add_interceptor_fn(func)
    }
}

impl CommonAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Method, Body> {
    /// Create a new CommonAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new(
    ) -> CommonAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
    {
        return CommonAPI::new_with_options(Arc::new(Mutex::new(SimpleAPI::new())));
    }
}

impl<Client, Req, Res, Method, B> CommonAPI<Client, Req, Res, Method, B> {
    pub fn make_api_response_only<R>(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIResponseOnly<R, Client, Req, Res, Method, B> {
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
    ) -> APINoBody<R, Client, Req, Res, Method, B> {
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
        request_serializer: Arc<dyn BodySerializer<T, B>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIHasBody<T, R, Client, Req, Res, Method, B> {
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
}

impl<C> CommonAPI<Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, Body> {
    #[cfg(feature = "multipart")]
    pub fn make_api_multipart<R>(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        // request_serializer: Arc<dyn BodySerializer<FormData, (String, Body)>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIMultipart<
        FormData,
        R,
        Client<C, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        Body,
    > {
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

impl<C, B> CommonAPI<Client<C, B>, Request<B>, Result<Response<B>>, Method, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    async fn _call_common(
        &self,
        method: Method,
        header: Option<HeaderMap>,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: B,
    ) -> StdResult<Box<B>, Box<dyn StdError>> {
        let simple_api = self.simple_api.lock().unwrap();
        let mut req = simple_api.make_request(
            method,
            relative_url,
            content_type,
            path_param,
            query_param,
            body,
        )?;

        if let Some(header) = header {
            let header_existing = req.headers_mut();
            for (k, v) in header.iter() {
                header_existing.insert(k, v.clone());
            }
        }

        let body = simple_api.simple_http.request(req).await??.into_body();

        Ok(Box::new(body))
    }

    pub async fn do_request(
        &self,
        method: Method,
        header: Option<HeaderMap>,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: B,
    ) -> StdResult<Box<B>, Box<dyn StdError>> {
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

impl<C> CommonAPI<Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
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

// APIResponseOnly API with only response options
// R: Response body Type
pub struct APIResponseOnly<R, Client, Req, Res, Method, B>(
    APINoBody<R, Client, Req, Res, Method, B>,
);
impl<R, C> APIResponseOnly<R, Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
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
        self.call_with_options(None, None::<QueryParam>).await
    }
    pub async fn call_with_options(
        &self,
        header: Option<HeaderMap>,
        query_param: Option<impl Into<QueryParam>>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.0
            .call_with_options(header, None::<PathParam>, query_param)
            .await
    }
}

// APINoBody API without request body options
// R: Response body Type
pub struct APINoBody<R, Client, Req, Res, Method, B> {
    base: CommonAPI<Client, Req, Res, Method, B>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<R, C> APINoBody<R, Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(&self, path_param: Option<PathParam>) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.call_with_options(None, path_param, None::<QueryParam>)
            .await
    }

    pub async fn call_with_options(
        &self,
        header: Option<HeaderMap>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
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
                query_param,
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
pub struct APIHasBody<T, R, Client, Req, Res, Method, B> {
    base: CommonAPI<Client, Req, Res, Method, B>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub request_serializer: Arc<dyn BodySerializer<T, B>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, C> APIHasBody<T, R, Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(
        &self,
        path_param: Option<impl Into<PathParam>>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.call_with_options(None, path_param, None::<QueryParam>, sent_body)
            .await
    }

    pub async fn call_with_options(
        &self,
        header: Option<HeaderMap>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
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
                query_param,
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
pub struct APIMultipart<T, R, Client, Req, Res, Method, B> {
    base: CommonAPI<Client, Req, Res, Method, B>,
    pub method: Method,
    pub relative_url: String,
    // pub content_type: String,
    pub request_serializer: Arc<dyn BodySerializer<T, (String, B)>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, C>
    APIMultipart<T, R, Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
where
    C: Connect + Clone + Send + Sync + 'static,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(
        &self,
        path_param: Option<impl Into<PathParam>>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        Body: Default,
    {
        self.call_with_options(None, path_param, None::<QueryParam>, sent_body)
            .await
    }

    pub async fn call_with_options(
        &self,
        header: Option<HeaderMap>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
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
                query_param,
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

// SimpleAPI SimpleAPI inspired by Retrofits
pub struct SimpleAPI<Client, Req, Res, Method, B> {
    pub simple_http: SimpleHTTP<Client, Req, Res, Method, B>,
    pub base_url: Url,
    pub default_header: HeaderMap,
}

impl<Client, Req, Res, Method, B> SimpleAPI<Client, Req, Res, Method, B> {
    pub fn new_with_options(
        simple_http: SimpleHTTP<Client, Req, Res, Method, B>,
        base_url: Url,
    ) -> Self {
        SimpleAPI {
            simple_http,
            base_url,
            default_header: HeaderMap::new(),
        }
    }
}

impl SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Method, Body> {
    /// Create a new SimpleAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new(
    ) -> SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
    {
        return SimpleAPI::new_with_options(
            SimpleHTTP::new(),
            Url::parse("http://localhost").ok().unwrap(),
        );
    }
}
impl Default
    for SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
{
    fn default(
    ) -> SimpleAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
    {
        SimpleAPI::<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, Method, Body>::new()
    }
}

impl<C, B> SimpleAPI<Client<C, B>, Request<B>, Result<Response<B>>, Method, B>
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
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: B,
    ) -> StdResult<Request<B>, Box<dyn StdError>> {
        let mut relative_url = relative_url.into();
        if let Some(path_param) = path_param {
            for (k, v) in path_param.into().into_iter() {
                relative_url = relative_url.replace(&("{".to_string() + &k + "}"), &v);
            }
        }

        let mut req = Request::new(body);
        // Url
        match self.base_url.join(&relative_url) {
            Ok(mut url) => {
                if let Some(query_param) = query_param {
                    for (k, v) in query_param.into().into_iter() {
                        url.set_query(Some(&(k + "=" + &v)));
                    }
                }
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
impl<C> SimpleAPI<Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, Body>
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

// #[inline]
// #[derive(Debug, Clone)]
