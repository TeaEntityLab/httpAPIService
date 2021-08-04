/*!
In this module there're implementations & tests of `SimpleHTTP`.
*/

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use http::method::Method;
// use futures::TryStreamExt;
// use hyper::body::HttpBody;
use bytes::Bytes;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, Client, Error, HeaderMap, Request, Response, Result, Uri};
use url::Url;

use super::common::{PathParam, QueryParam};
use super::simple_api::{BaseAPI, BaseService, BodySerializer, SimpleAPI};
use super::simple_http::{
    BaseClient, FormDataParseError, SimpleHTTP, SimpleHTTPResponse, DEFAULT_TIMEOUT_MILLISECOND,
};

#[cfg(feature = "for_serde")]
pub use super::simple_api::DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES;

#[cfg(feature = "multipart")]
pub use super::simple_api::{DEFAULT_MULTIPART_SERIALIZER, DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES};
#[cfg(feature = "multipart")]
pub use super::simple_http::{
    data_and_boundary_from_multipart, get_content_type_from_multipart_boundary,
};
#[cfg(feature = "multipart")]
use formdata::FormData;
#[cfg(feature = "multipart")]
use multer;
#[cfg(feature = "multipart")]
use multer::Multipart;

pub struct HyperClient<C, B>(Client<C, B>);
impl<C, B> BaseClient<Client<C, B>, Request<B>, Result<Response<Body>>, Method, HeaderMap, B>
    for HyperClient<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn request(&self, req: Request<B>) -> Pin<Box<dyn Future<Output = Result<Response<Body>>>>> {
        Box::pin(self.0.request(req))
    }
    fn get_client(&mut self) -> &mut Client<C, B> {
        return &mut self.0;
    }
}

pub struct HyperSimpleAPI<Client, Req, Res, Header, B>(
    SimpleAPI<Client, Req, Res, Method, Header, B>,
);
impl<Client, Req, Res, B> BaseAPI<Client, Req, Res, Method, HeaderMap, B>
    for HyperSimpleAPI<Client, Req, Res, HeaderMap, B>
{
    fn set_base_url(&mut self, url: Url) {
        self.0.base_url = url;
    }
    fn get_base_url(&self) -> Url {
        self.0.base_url.clone()
    }
    fn set_default_header(&mut self, header: Option<HeaderMap>) {
        self.0.default_header = header;
    }
    fn get_default_header(&self) -> Option<HeaderMap> {
        self.0.default_header.clone()
    }

    fn get_simple_http(&mut self) -> &mut SimpleHTTP<Client, Req, Res, Method, HeaderMap, B> {
        &mut self.0.simple_http
    }
}

impl
    SimpleHTTP<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    >
{
    /// Create a new SimpleHTTP with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new_for_hyper() -> SimpleHTTP<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    > {
        return SimpleHTTP::new_with_options(
            Arc::new(Mutex::new(
                HyperClient::<HttpConnector, Body>(Client::new()),
            )),
            VecDeque::new(),
            DEFAULT_TIMEOUT_MILLISECOND,
        );
    }
}
impl Default
    for SimpleHTTP<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    >
{
    fn default() -> SimpleHTTP<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    > {
        SimpleHTTP::new_for_hyper()
    }
}

impl
    SimpleAPI<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    >
{
    /// Create a new SimpleAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new_for_hyper() -> SimpleAPI<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    > {
        return SimpleAPI::new_with_options(
            SimpleHTTP::new_for_hyper(),
            Url::parse("http://localhost").ok().unwrap(),
        );
    }
}

impl Default
    for SimpleAPI<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    >
{
    fn default() -> SimpleAPI<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        Method,
        HeaderMap,
        Body,
    > {
        SimpleAPI::<
            Client<HttpConnector, Body>,
            Request<Body>,
            Result<Response<Body>>,
            Method,
            HeaderMap,
            Body,
        >::new_for_hyper()
    }
}

#[derive(Debug)]
pub struct HyperError(Error);
impl StdError for HyperError {}

impl std::fmt::Display for HyperError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
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
// #[derive(Clone)]
pub struct CommonAPI<Client, Req, Res, Header, B> {
    pub simple_api: Arc<Mutex<dyn BaseAPI<Client, Req, Res, Method, Header, B>>>,
}

impl<Client, Req, Res, Header, B> Clone for CommonAPI<Client, Req, Res, Header, B> {
    fn clone(&self) -> Self {
        CommonAPI {
            simple_api: self.simple_api.clone(),
        }
    }
}

impl<Client, Req, Res, Header, B> CommonAPI<Client, Req, Res, Header, B> {
    pub fn new_with_options(
        simple_api: Arc<Mutex<dyn BaseAPI<Client, Req, Res, Method, Header, B>>>,
    ) -> Self {
        Self { simple_api }
    }

    pub fn new_copy(&self) -> Box<CommonAPI<Client, Req, Res, Header, B>> {
        return Box::new(self.clone());
    }
}

impl
    CommonAPI<Client<HttpConnector, Body>, Request<Body>, Result<Response<Body>>, HeaderMap, Body>
{
    /// Create a new CommonAPI with a Client with the default [config](Builder).
    ///
    /// # Note
    ///
    /// The default connector does **not** handle TLS. Speaking to `https`
    /// destinations will require [configuring a connector that implements
    /// TLS](https://hyper.rs/guides/client/configuration).
    #[inline]
    pub fn new_for_hyper() -> CommonAPI<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        HeaderMap,
        Body,
    > {
        return CommonAPI::new_with_options(Arc::new(Mutex::new(HyperSimpleAPI(
            SimpleAPI::new_for_hyper(),
        ))));
    }
}

impl Default
    for CommonAPI<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        HeaderMap,
        Body,
    >
{
    fn default() -> CommonAPI<
        Client<HttpConnector, Body>,
        Request<Body>,
        Result<Response<Body>>,
        HeaderMap,
        Body,
    > {
        CommonAPI::new_for_hyper()
    }
}

impl<C, B> dyn BaseService<Client<C, B>, Request<B>, Result<Response<B>>, Method, HeaderMap, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
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
            relative_url.into(),
            content_type.into(),
            if let Some(v) = path_param {
                Some(v.into())
            } else {
                None
            },
            if let Some(v) = query_param {
                Some(v.into())
            } else {
                None
            },
            body,
        )
        .await
    }

    pub async fn do_request_multipart(
        &self,
        method: Method,
        header: Option<HeaderMap>,
        relative_url: impl Into<String>,
        // content_type: impl Into<String>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: FormData,
    ) -> StdResult<Box<B>, Box<dyn StdError>>
    where
        B: From<Bytes>,
    {
        let (content_type, body) = DEFAULT_MULTIPART_SERIALIZER.encode(body)?;
        self._call_common(
            method,
            header,
            relative_url.into(),
            content_type,
            if let Some(v) = path_param {
                Some(v.into())
            } else {
                None
            },
            if let Some(v) = query_param {
                Some(v.into())
            } else {
                None
            },
            body,
        )
        .await
    }
}

impl<C, B> CommonAPI<Client<C, B>, Request<B>, Result<Response<B>>, HeaderMap, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub fn as_base_service_shared(
        &self,
    ) -> Arc<dyn BaseService<Client<C, B>, Request<B>, Result<Response<B>>, Method, HeaderMap, B>>
    {
        Arc::new(*self.new_copy())
    }
    pub fn as_base_service_setter(
        &self,
    ) -> Box<dyn BaseService<Client<C, B>, Request<B>, Result<Response<B>>, Method, HeaderMap, B>>
    {
        self.new_copy()
    }
}

impl<C, B> BaseService<Client<C, B>, Request<B>, Result<Response<B>>, Method, HeaderMap, B>
    for CommonAPI<Client<C, B>, Request<B>, Result<Response<B>>, HeaderMap, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn body_to_bytes(
        &self,
        body: B,
    ) -> Pin<Box<dyn Future<Output = StdResult<Bytes, Box<dyn StdError + Send + Sync>>>>> {
        Box::pin(async {
            match hyper::body::to_bytes(body).await {
                Ok(v) => Ok(v),
                Err(e) => Err(e.into()),
            }
        })
    }

    fn get_simple_api(
        &self,
    ) -> &Arc<Mutex<dyn BaseAPI<Client<C, B>, Request<B>, Result<Response<B>>, Method, HeaderMap, B>>>
    {
        &self.simple_api
    }

    fn _call_common(
        &self,
        method: Method,
        header: Option<HeaderMap>,
        relative_url: String,
        content_type: String,
        path_param: Option<PathParam>,
        query_param: Option<QueryParam>,
        body: B,
    ) -> Pin<Box<dyn Future<Output = StdResult<Box<B>, Box<dyn StdError>>>>> {
        let simple_api = self.simple_api.clone();

        Box::pin(async move {
            let mut simple_api = simple_api.lock().unwrap();
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

            let body = simple_api
                .get_simple_http()
                .request(req)
                .await??
                .into_body();

            Ok(Box::new(body))
        })
    }
}

impl<C, B> dyn BaseAPI<Client<C, B>, Request<B>, Result<Response<B>>, Method, HeaderMap, B>
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
        match self.get_base_url().join(&relative_url) {
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
        if let Some(header) = self.get_default_header() {
            *req.headers_mut() = header.clone();
        }
        let content_type = content_type.into();
        if !content_type.is_empty() {
            req.headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_str(&content_type)?);
        }

        Ok(req)
    }

    #[cfg(feature = "multipart")]
    pub fn make_request_multipart(
        &self,
        method: Method,
        relative_url: impl Into<String>,
        // content_type: String,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: FormData,
    ) -> StdResult<Request<B>, Box<dyn StdError>>
    where
        B: From<Bytes>,
    {
        let (content_type, body) = DEFAULT_MULTIPART_SERIALIZER.encode(body)?;
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

impl<C, B> SimpleHTTP<Client<C, B>, Request<B>, Result<Response<B>>, Method, HeaderMap, B>
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
            self.get_timeout_duration(),
            { self.client.lock().unwrap() }.request(request),
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
