/*!
In this module there're implementations & tests of `SimpleHTTP`.
*/

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::future::Future;
use std::io::{self, Read, Write};
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;

// use futures::TryStreamExt;
use bytes::{Buf, Bytes};
use futures::executor::ThreadPool;
use futures::prelude::*;
use futures::stream;
use futures::task::SpawnExt;
use ureq::{Agent, Header, Request, Response};
use url::Url;

use super::common::{PathParam, QueryParam};
use super::simple_api::{BaseAPI, BaseService, BodySerializer, SimpleAPI};
use super::simple_http::{BaseClient, SimpleHTTP, SimpleHTTPResponse, DEFAULT_TIMEOUT_MILLISECOND};
use fp_rust::common::shared_thread_pool;

#[cfg(feature = "for_serde")]
pub use super::simple_api::DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES;

#[cfg(feature = "multipart")]
pub use super::simple_api::{DEFAULT_MULTIPART_SERIALIZER, DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES};
#[cfg(feature = "multipart")]
pub use super::simple_http::{
    data_and_boundary_from_multipart, get_content_type_from_multipart_boundary, FormDataParseError,
};
#[cfg(feature = "multipart")]
use formdata::FormData;
#[cfg(feature = "multipart")]
use multer;
#[cfg(feature = "multipart")]
use multer::Multipart;

pub const CONTENT_TYPE: &'static str = "content-type";

#[derive(Clone)]
pub struct WriteForBody {
    // pub Box<Sender>
    pub cached: Arc<Mutex<VecDeque<Bytes>>>,
    pub alive: Arc<Mutex<AtomicBool>>,
}

impl WriteForBody {
    pub fn close(&self) {
        self.alive.lock().unwrap().store(false, Ordering::SeqCst);
    }
}

impl Read for WriteForBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        {
            if !self.alive.lock().unwrap().load(Ordering::SeqCst) {
                return Ok(0);
            }
        }

        {
            let mut cached = self.cached.lock().unwrap();
            if cached.is_empty() {
                return Ok(0);
            }

            let item = cached.pop_front();
            if let Some(item) = item {
                // let len = item.len();
                match item.reader().read(buf) {
                    Ok(len) => return Ok(len),
                    Err(e) => return Err(e),
                }
            }

            return Ok(0);
        }
    }
}
impl Write for WriteForBody {
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
impl BodySerializer<FormData, (String, Box<dyn Read + Send + Sync>)>
    for MultipartSerializerForStream
where
// B: HttpBody + Send + 'static,
// B: From<Body>,
// B::Data: Send,
// B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn encode(
        &self,
        origin: FormData,
    ) -> StdResult<(String, Box<dyn Read + Send + Sync>), Box<dyn StdError>> {
        let boundary = formdata::generate_boundary();
        let boundary_thread = boundary.clone();

        let data = WriteForBody {
            cached: Arc::new(Mutex::new(VecDeque::new())),
            alive: Arc::new(Mutex::new(AtomicBool::new(true))),
        };
        let mut data_thread = data.clone();

        let _ = thread::spawn(move || {
            match formdata::write_formdata(&mut data_thread, &boundary_thread, &origin) {
                Err(e) => println!("Error -> write_formdata {:?}", e),
                _ => {}
            };
        });
        let content_type = get_content_type_from_multipart_boundary(boundary)?;

        Ok((content_type, Box::new(data)))
    }
}
#[cfg(feature = "multipart")]
#[allow(dead_code)]
pub(crate) const DEFAULT_MULTIPART_SERIALIZER_FOR_STREAM: MultipartSerializerForStream =
    MultipartSerializerForStream { thread_pool: None };

pub struct UreqClient {
    pub agent: Agent,
    pub thread_pool: Option<ThreadPool>,
}

impl
    BaseClient<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    > for UreqClient
{
    fn get_client(&mut self) -> &mut Agent {
        return &mut self.agent;
    }

    fn request(
        &self,
        req: (Request, Option<Bytes>),
    ) -> Pin<Box<dyn Future<Output = Result<Response, Box<dyn StdError>>>>> {
        // let req = self.0.get("path");
        let spawn_future_result = match &self.thread_pool {
            Some(thread_pool) => thread_pool.spawn_with_handle(async {
                match req.1 {
                    Some(body) => req.0.send_bytes(&body),
                    None => req.0.call(),
                }
            }),
            None => shared_thread_pool()
                .inner
                .lock()
                .unwrap()
                .spawn_with_handle(async {
                    match req.1 {
                        Some(body) => req.0.send_bytes(&body),
                        None => req.0.call(),
                    }
                }),
        };

        Box::pin(async {
            match spawn_future_result {
                Ok(future) => match future.await {
                    Ok(v) => Ok(v),
                    Err(e) => Err(Box::new(e) as Box<dyn StdError>),
                },
                Err(e) => Err(Box::new(e) as Box<dyn StdError>),
            }
        })
    }
}

pub struct UreqSimpleAPI<Client, Req, Res, Method, Header, Bytes>(
    SimpleAPI<Client, Req, Res, Method, Header, Bytes>,
);
impl<Client, Req, Res, Bytes> BaseAPI<Client, Req, Res, String, Vec<Header>, Bytes>
    for UreqSimpleAPI<Client, Req, Res, String, Vec<Header>, Bytes>
{
    fn set_base_url(&mut self, url: Url) {
        self.0.base_url = url;
    }
    fn get_base_url(&self) -> Url {
        self.0.base_url.clone()
    }
    fn set_default_header(&mut self, header: Option<Vec<Header>>) {
        self.0.default_header = header;
    }
    fn get_default_header(&self) -> Option<Vec<Header>> {
        self.0.default_header.clone()
    }

    fn get_simple_http(&mut self) -> &mut SimpleHTTP<Client, Req, Res, String, Vec<Header>, Bytes> {
        &mut self.0.simple_http
    }
}

impl
    SimpleHTTP<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    /// Create a new SimpleHTTP with a Client with the default [config](Builder).
    #[inline]
    pub fn new_for_ureq() -> SimpleHTTP<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    > {
        return SimpleHTTP::new_with_options(
            Arc::new(Mutex::new(UreqClient {
                agent: Agent::new(),
                thread_pool: None,
            })),
            VecDeque::new(),
            DEFAULT_TIMEOUT_MILLISECOND,
        );
    }
}
impl Default
    for SimpleHTTP<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    fn default() -> SimpleHTTP<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    > {
        SimpleHTTP::new_for_ureq()
    }
}

impl
    SimpleAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    /// Create a new SimpleAPI with a Client with the default [config](Builder).
    #[inline]
    pub fn new_for_ureq() -> SimpleAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    > {
        return SimpleAPI::new_with_options(
            SimpleHTTP::new_for_ureq(),
            Url::parse("http://localhost").ok().unwrap(),
        );
    }
}

impl Default
    for SimpleAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    fn default() -> SimpleAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    > {
        SimpleAPI::<
            Agent,
            (Request, Option<Bytes>),
            Result<Response, Box<dyn StdError>>,
            String,
            Vec<Header>,
            Bytes,
        >::new_for_ureq()
    }
}
//
// #[derive(Debug)]
// pub struct UreqError(Error);
// impl StdError for UreqError {}
//
// impl std::fmt::Display for UreqError {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         self.0.fmt(f)
//     }
// }

/*
`CommonAPI` implements `make_api_response_only()`/`make_api_no_body()`/`make_api_has_body()`,
for Retrofit-like usages.
# Arguments
#
# Remarks
It's inspired by `Retrofit`.
*/
// #[derive(Clone)]
pub struct CommonAPI<Client, Req, Res, Method, Header, Bytes> {
    pub simple_api: Arc<Mutex<dyn BaseAPI<Client, Req, Res, Method, Header, Bytes>>>,
}

impl<Client, Req, Res, Method, Header, Bytes> Clone
    for CommonAPI<Client, Req, Res, Method, Header, Bytes>
{
    fn clone(&self) -> Self {
        CommonAPI {
            simple_api: self.simple_api.clone(),
        }
    }
}

impl<Client, Req, Res, Method, Header, Bytes> CommonAPI<Client, Req, Res, Method, Header, Bytes> {
    pub fn new_with_options(
        simple_api: Arc<Mutex<dyn BaseAPI<Client, Req, Res, Method, Header, Bytes>>>,
    ) -> Self {
        Self { simple_api }
    }

    pub fn new_copy(&self) -> Box<CommonAPI<Client, Req, Res, Method, Header, Bytes>> {
        return Box::new(self.clone());
    }
}

impl
    CommonAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    /// Create a new CommonAPI with a Client with the default [config](Builder).
    #[inline]
    pub fn new_for_ureq() -> CommonAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    > {
        return CommonAPI::new_with_options(Arc::new(Mutex::new(UreqSimpleAPI(
            SimpleAPI::new_for_ureq(),
        ))));
    }
}

impl Default
    for CommonAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    fn default() -> CommonAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    > {
        CommonAPI::new_for_ureq()
    }
}

impl
    dyn BaseService<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    pub async fn do_request(
        &self,
        method: String,
        header: Option<Vec<Header>>,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: Bytes,
    ) -> StdResult<Box<Bytes>, Box<dyn StdError>> {
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
        method: String,
        header: Option<Vec<Header>>,
        relative_url: impl Into<String>,
        // content_type: impl Into<String>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: FormData,
    ) -> StdResult<Box<Bytes>, Box<dyn StdError>> {
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

impl
    CommonAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    pub fn as_base_service_shared(
        &self,
    ) -> Arc<
        dyn BaseService<
            Agent,
            (Request, Option<Bytes>),
            Result<Response, Box<dyn StdError>>,
            String,
            Vec<Header>,
            Bytes,
        >,
    > {
        Arc::new(*self.new_copy())
    }
    pub fn as_base_service_setter(
        &self,
    ) -> Box<
        dyn BaseService<
            Agent,
            (Request, Option<Bytes>),
            Result<Response, Box<dyn StdError>>,
            String,
            Vec<Header>,
            Bytes,
        >,
    > {
        self.new_copy()
    }
}

impl
    BaseService<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
    for CommonAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    fn body_to_bytes(
        &self,
        body: Bytes,
    ) -> Pin<Box<dyn Future<Output = StdResult<Bytes, Box<dyn StdError + Send + Sync>>>>> {
        Box::pin(async { Ok(body) })
    }

    fn get_simple_api(
        &self,
    ) -> &Arc<
        Mutex<
            dyn BaseAPI<
                Agent,
                (Request, Option<Bytes>),
                Result<Response, Box<dyn StdError>>,
                String,
                Vec<Header>,
                Bytes,
            >,
        >,
    > {
        &self.simple_api
    }

    fn _call_common(
        &self,
        method: String,
        header: Option<Vec<Header>>,
        relative_url: String,
        content_type: String,
        path_param: Option<PathParam>,
        query_param: Option<QueryParam>,
        body: Bytes,
    ) -> Pin<Box<dyn Future<Output = StdResult<Box<Bytes>, Box<dyn StdError>>>>> {
        let simple_api = self.simple_api.clone();

        Box::pin(async move {
            let mut simple_api = simple_api.lock().unwrap();
            let (mut req, body) = simple_api.make_request(
                method,
                relative_url,
                content_type,
                path_param,
                query_param,
                body,
            )?;

            if let Some(header) = header {
                for item in header.into_iter() {
                    if let Some(v) = item.value() {
                        req = req.set(item.name(), v);
                    }
                }
            }

            let res = simple_api.get_simple_http().request((req, body)).await??;
            let mut bytes: Vec<u8> = Vec::with_capacity(1_000);
            res.into_reader().take(10_000_000).read_to_end(&mut bytes)?;

            Ok(Box::new(Bytes::from(bytes)))
        })
    }
}

impl
    dyn BaseAPI<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    pub fn make_request(
        &mut self,
        method: String,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: Bytes,
    ) -> StdResult<(Request, Option<Bytes>), Box<dyn StdError>> {
        let mut relative_url = relative_url.into();
        if let Some(path_param) = path_param {
            for (k, v) in path_param.into().into_iter() {
                relative_url = relative_url.replace(&("{".to_string() + &k + "}"), &v);
            }
        }

        // Url
        let uri: String;
        match self.get_base_url().join(&relative_url) {
            Ok(mut url) => {
                if let Some(query_param) = query_param {
                    for (k, v) in query_param.into().into_iter() {
                        url.set_query(Some(&(k + "=" + &v)));
                    }
                }
                uri = url.into();
            }
            Err(e) => return Err(Box::new(e)),
        };
        // Method

        let mut req = { self.get_simple_http().client.lock().unwrap() }
            .get_client()
            .request(&method, &uri);
        req = req.timeout(self.get_simple_http().get_timeout_duration());

        // Header
        if let Some(header) = self.get_default_header() {
            for item in header.into_iter() {
                if let Some(v) = item.value() {
                    req = req.set(item.name(), v);
                }
            }
        }
        let content_type = content_type.into();
        if !content_type.is_empty() {
            req = req.set(CONTENT_TYPE, &content_type);
        }

        Ok((req, Some(body)))
    }

    #[cfg(feature = "multipart")]
    pub fn make_request_multipart(
        &mut self,
        method: String,
        relative_url: impl Into<String>,
        // content_type: String,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        body: FormData,
    ) -> StdResult<(Request, Option<Bytes>), Box<dyn StdError>> {
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
    mut header_map: Vec<Header>,
    token: impl Into<String>,
) -> StdResult<Vec<Header>, Box<dyn StdError>> {
    let str = token.into();
    header_map.push(Header::new("Authorization", &str));

    Ok(header_map)
}

pub fn add_header_authentication_bearer(
    header_map: Vec<Header>,
    token: impl Into<String>,
) -> StdResult<Vec<Header>, Box<dyn StdError>> {
    return add_header_authentication(header_map, "Bearer ".to_string() + &token.into());
}

#[cfg(feature = "multipart")]
pub fn body_from_multipart(form_data: &FormData) -> StdResult<(Bytes, Vec<u8>), Box<dyn StdError>> {
    let (data, boundary) = data_and_boundary_from_multipart(form_data)?;

    Ok((Bytes::from(data), boundary))
}
#[cfg(feature = "multipart")]
pub async fn body_to_multipart(
    headers: &Vec<Header>,
    body: Bytes,
) -> StdResult<Multipart<'_>, Box<dyn StdError>> {
    let boundary: String;

    let body = stream::iter(vec![body])
        .map(|y| -> StdResult<Bytes, Box<dyn std::error::Error + Send + Sync>> { Ok(y) });

    for item in headers.into_iter() {
        if item.name() == CONTENT_TYPE {
            if let Some(content_type) = item.value() {
                boundary = multer::parse_boundary(&content_type)?;
                return Ok(Multipart::new(body, boundary));
            }
        }
    }

    Err(Box::new(FormDataParseError::new(
        "{}: None".to_string() + CONTENT_TYPE,
    )))
}

impl
    SimpleHTTP<
        Agent,
        (Request, Option<Bytes>),
        Result<Response, Box<dyn StdError>>,
        String,
        Vec<Header>,
        Bytes,
    >
{
    pub async fn request(
        &self,
        mut request: (Request, Option<Bytes>),
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        for interceptor in &mut self.interceptors.iter() {
            interceptor.intercept(&mut request)?;
        }

        // Implement timeout
        match { self.client.lock().unwrap().request(request) }.await {
            Ok(result) => Ok(Ok(result)),
            Err(e) => Err(e),
        }
    }

    pub async fn get(
        &self,
        uri: impl Into<String>,
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        let req = { self.client.lock().unwrap().get_client().get(&uri.into()) };
        self.request((req, None)).await
    }
    pub async fn head(
        &self,
        uri: impl Into<String>,
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        let req = { self.client.lock().unwrap().get_client().head(&uri.into()) };
        self.request((req, None)).await
    }
    pub async fn option(
        &self,
        uri: impl Into<String>,
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        let req = {
            { self.client.lock().unwrap() }
                .get_client()
                .request("OPTIONS", &uri.into())
        };
        self.request((req, None)).await
    }
    pub async fn delete(
        &self,
        uri: impl Into<String>,
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        let req = { self.client.lock().unwrap().get_client().delete(&uri.into()) };
        self.request((req, None)).await
    }

    pub async fn post(
        &self,
        uri: impl Into<String>,
        body: Bytes,
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        let req = { self.client.lock().unwrap().get_client().post(&uri.into()) };
        self.request((req, Some(body))).await
    }
    pub async fn put(
        &self,
        uri: impl Into<String>,
        body: Bytes,
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        let req = { self.client.lock().unwrap().get_client().put(&uri.into()) };
        self.request((req, Some(body))).await
    }
    pub async fn patch(
        &self,
        uri: impl Into<String>,
        body: Bytes,
    ) -> SimpleHTTPResponse<Result<Response, Box<dyn StdError>>> {
        let req = {
            { self.client.lock().unwrap() }
                .get_client()
                .request("PATCH", &uri.into())
        };
        self.request((req, Some(body))).await
    }
}
