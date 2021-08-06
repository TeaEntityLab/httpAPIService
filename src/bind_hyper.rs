/*!
In this module there're implementations & tests of `SimpleHTTP`.
*/

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::future::Future;
use std::io::{self, Write};
use std::pin::Pin;
use std::result::Result as StdResult;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::task::{Context, Poll, Waker};
use std::thread;

use http::method::Method;
// use futures::TryStreamExt;
// use hyper::body::HttpBody;
use bytes::Bytes;
// use futures::executor::block_on;
use futures::executor::ThreadPool;
use futures::prelude::*;
use futures::Stream;
// use futures::task::SpawnExt;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, Client, HeaderMap, Request, Response, Result, Uri};
use url::Url;

use super::common::{make_stream, PathParam, QueryParam, WriteForStream};
use super::simple_api::{
    APIMultipart, BaseAPI, BaseService, BodyDeserializer, BodySerializer, SimpleAPI,
};
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

#[derive(Clone)]
pub struct WriteForBody {
    // pub Box<Sender>
    pub cached: Arc<Mutex<VecDeque<Bytes>>>,
    pub waker: Arc<Mutex<Option<Waker>>>,
    pub alive: Arc<Mutex<AtomicBool>>,
}

impl WriteForBody {
    pub fn close(&self) {
        self.alive.lock().unwrap().store(false, Ordering::SeqCst);

        {
            if let Some(waker) = self.waker.lock().unwrap().take() {
                waker.wake()
            }
        }
    }
}

impl Stream for WriteForBody {
    type Item = StdResult<Bytes, Box<dyn StdError + Send + Sync>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        {
            {
                let mut cached = self.cached.lock().unwrap();
                if !cached.is_empty() {
                    self.waker.lock().unwrap().replace(cx.waker().clone());

                    let d = cached.pop_front();
                    println!("WriteForBody stream read content: {:?}", d.clone());
                    return Poll::Ready(Some(Ok(d.unwrap())));
                }
            }
            {
                if !self.alive.lock().unwrap().load(Ordering::SeqCst) {
                    println!("WriteForBody stream end");
                    return Poll::Ready(None);
                }
            }
        }

        {
            self.waker.lock().unwrap().replace(cx.waker().clone());
            println!("WriteForBody stream pending");
            Poll::Pending
        }
    }
}

impl io::Write for WriteForBody {
    fn write(&mut self, d: &[u8]) -> io::Result<usize> {
        let len = d.len();
        println!("WriteForBody write len: {:?}", len);
        if len <= 0 {
            return Ok(len);
        }
        let d = Bytes::from(d.to_vec());
        println!("WriteForBody write content: {:?}", d.clone());

        {
            let mut cached = self.cached.lock().unwrap();
            cached.push_back(d);
            cached.reserve_exact(10);
        }
        {
            if let Some(waker) = self.waker.lock().unwrap().take() {
                waker.wake();
            }
        }

        /*
        match self.0.try_send_data(d) {
            Ok(_) => {
                println!("WriteForBody write ok");
            }
            Err(_) => {
                println!("WriteForBody write error");
            }
        }
        */
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        {
            if let Some(waker) = self.waker.lock().unwrap().take() {
                waker.wake();
            }
        }

        println!("flush ok");
        Ok(())
    }
}

#[cfg(feature = "multipart")]
#[derive(Debug, Clone)]
/// MultipartSerializerForStream Serialize the multipart body (for put/post/patch etc)
pub struct MultipartSerializerForStream {
    // NOTE: It can't be Copy because of this one:
    thread_pool: Option<Arc<ThreadPool>>,
}
#[cfg(feature = "multipart")]
impl BodySerializer<FormData, (String, Body)> for MultipartSerializerForStream
where
// B: HttpBody + Send + 'static,
// B: From<Body>,
// B::Data: Send,
// B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn encode(&self, origin: FormData) -> StdResult<(String, Body), Box<dyn StdError>> {
        // let mut data = Vec::<u8>::new();

        let (tx, rx) = make_stream::<Bytes>();
        let mut data = WriteForStream(tx);

        /*
        let (tx, body) = Body::channel();
        let mut data = WriteForBody(Box::new(tx));
        // */

        /*
        let mut data = WriteForBody {
            cached: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
            waker: Arc::new(Mutex::new(None)),
            alive: Arc::new(Mutex::new(AtomicBool::new(true))),
        };
        let body = data.clone();
        */

        let boundary = formdata::generate_boundary();
        let boundary_thread = boundary.clone();
        //*
        // println!("Enter encode");
        let _ = thread::spawn(move || {
            // let _ = tokio::spawn(async move {
            // let _ = tokio::spawn(async move {
            // tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            // thread::sleep_ms(2000);
            // data.0.send_data(Bytes::new()).await;

            // println!("spawn: Some");
            // println!("write_formdata begin");
            match formdata::write_formdata(&mut data, &boundary_thread, &origin) {
                Err(e) => println!("Error -> write_formdata {:?}", e),
                _ => {}
            };
            // println!("write_formdata done");

            match data.flush() {
                Err(e) => println!("Error -> flush {:?}", e),
                _ => {}
            };
            // println!("flush ok");

            let mut tx = data.0;
            tx.close_channel();
            drop(tx);
            // println!("Close!!");
        });
        // */
        let content_type = get_content_type_from_multipart_boundary(boundary)?;

        let body = rx
            .map(|y| Ok::<Bytes, Box<dyn StdError + Send + Sync>>(y))
            .into_stream();

        // Ok((content_type, B::from(body)))
        // Ok((content_type, body))
        Ok((content_type, Body::wrap_stream(body)))
    }
}
#[cfg(feature = "multipart")]
pub const DEFAULT_MULTIPART_SERIALIZER_FOR_STREAM: MultipartSerializerForStream =
    MultipartSerializerForStream { thread_pool: None };

pub struct HyperClient<C, B> {
    pub client: Client<C, B>,
    pub thread_pool: Option<ThreadPool>,
}
impl<C, B> BaseClient<Client<C, B>, Request<B>, Result<Response<Body>>, Method, HeaderMap, B>
    for HyperClient<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn request(&self, req: Request<B>) -> Pin<Box<dyn Future<Output = Result<Response<Body>>>>> {
        Box::pin(self.client.request(req))
    }
    fn get_client(&mut self) -> &mut Client<C, B> {
        return &mut self.client;
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
            Arc::new(Mutex::new(HyperClient::<HttpConnector, Body> {
                client: Client::new(),
                thread_pool: None,
            })),
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

// #[derive(Debug)]
// pub struct HyperError(Error);
// impl StdError for HyperError {}
//
// impl std::fmt::Display for HyperError {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         self.0.fmt(f)
//     }
// }

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

    /*
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
    // */
}

impl<C>
    dyn BaseService<Client<C, Body>, Request<Body>, Result<Response<Body>>, Method, HeaderMap, Body>
{
    #[cfg(feature = "multipart")]
    // NOTE: Experimental
    pub fn make_api_multipart_for_stream<R>(
        &self,
        base: Arc<
            dyn BaseService<
                Client<C, Body>,
                Request<Body>,
                Result<Response<Body>>,
                Method,
                HeaderMap,
                Body,
            >,
        >,
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
        HeaderMap,
        Body,
    >
    where
        Body: From<Bytes>,
    {
        APIMultipart {
            base,
            method,
            relative_url: relative_url.into(),
            request_serializer: Arc::new(DEFAULT_MULTIPART_SERIALIZER_FOR_STREAM.clone()),
            response_deserializer,
        }
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
    ) -> StdResult<Box<Body>, Box<dyn StdError>>
    where
        Body: From<Bytes>,
    {
        let (content_type, body) = DEFAULT_MULTIPART_SERIALIZER_FOR_STREAM.encode(body)?;
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
