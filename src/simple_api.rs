/*!
In this module there're implementations & tests of `SimpleAPI`.
*/

use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use url::Url;

pub use super::common::{PathParam, QueryParam};
use super::simple_http::{
    data_and_boundary_from_multipart, get_content_type_from_multipart_boundary, BaseClient,
    Interceptor, InterceptorFunc, SimpleHTTP,
};

#[cfg(feature = "multipart")]
use formdata::FormData;

#[cfg(feature = "for_serde")]
use serde::{de::DeserializeOwned, Serialize};

/*
`BodySerializer  Serialize the body (for put/post/patch etc)
*/
pub trait BodySerializer<T, B> {
    fn encode(&self, origin: T) -> StdResult<B, Box<dyn StdError>>;
}
/*
`BodyDeserializer` Deserialize the body (for response)
*/
pub trait BodyDeserializer<R> {
    fn decode(&self, bytes: &Bytes) -> StdResult<Box<R>, Box<dyn StdError>>;
}

#[derive(Debug, Clone, Copy)]
// DummyBypassSerializerForBytes Dummy bypass the Bytes data, do nothing (for put/post/patch etc)
pub struct DummyBypassSerializerForBytes {}
impl BodySerializer<Bytes, Bytes> for DummyBypassSerializerForBytes {
    fn encode(&self, origin: Bytes) -> StdResult<Bytes, Box<dyn StdError>> {
        Ok(Bytes::from(origin))
    }
}
pub const DEFAULT_DUMMY_BYPASS_SERIALIZER_FOR_BYTES: DummyBypassSerializerForBytes =
    DummyBypassSerializerForBytes {};

#[derive(Debug, Clone, Copy)]
// DummyBypassSerializer Dummy bypass the body data, do nothing (for put/post/patch etc)
pub struct DummyBypassSerializer {}
impl<B> BodySerializer<Bytes, B> for DummyBypassSerializer
where
    B: From<Bytes>,
{
    fn encode(&self, origin: Bytes) -> StdResult<B, Box<dyn StdError>> {
        Ok(B::from(origin))
    }
}
pub const DEFAULT_DUMMY_BYPASS_SERIALIZER: DummyBypassSerializer = DummyBypassSerializer {};

#[derive(Debug, Clone, Copy)]
/*
DummyBypassDeserializer Dummy bypass the body, do nothing (for response)
*/
pub struct DummyBypassDeserializer {}
impl BodyDeserializer<Bytes> for DummyBypassDeserializer {
    fn decode(&self, bytes: &Bytes) -> StdResult<Box<Bytes>, Box<dyn StdError>> {
        Ok(Box::new(bytes.clone()))
    }
}
pub const DEFAULT_DUMMY_BYPASS_DESERIALIZER: DummyBypassDeserializer = DummyBypassDeserializer {};

#[cfg(feature = "multipart")]
#[derive(Debug, Clone, Copy)]
// MultipartSerializerForBytes Serialize the multipart body (for put/post/patch etc)
pub struct MultipartSerializerForBytes {}
#[cfg(feature = "multipart")]
impl BodySerializer<FormData, (String, Bytes)> for MultipartSerializerForBytes {
    fn encode(&self, origin: FormData) -> StdResult<(String, Bytes), Box<dyn StdError>> {
        let (body, boundary) = data_and_boundary_from_multipart(&origin)?;
        let content_type = get_content_type_from_multipart_boundary(boundary)?;

        Ok((content_type, Bytes::from(body)))
    }
}
#[cfg(feature = "multipart")]
pub const DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES: MultipartSerializerForBytes =
    MultipartSerializerForBytes {};

#[cfg(feature = "multipart")]
#[derive(Debug, Clone, Copy)]
// MultipartSerializer Serialize the multipart body (for put/post/patch etc)
pub struct MultipartSerializer {}
#[cfg(feature = "multipart")]
impl<B> BodySerializer<FormData, (String, B)> for MultipartSerializer
where
    B: From<Bytes>,
{
    fn encode(&self, origin: FormData) -> StdResult<(String, B), Box<dyn StdError>> {
        let (content_type, body) = DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES.encode(origin)?;

        Ok((content_type, B::from(body)))
    }
}
#[cfg(feature = "multipart")]
pub const DEFAULT_MULTIPART_SERIALIZER: MultipartSerializer = MultipartSerializer {};

#[cfg(feature = "for_serde")]
#[derive(Debug, Clone, Copy)]
// SerdeJsonSerializer Serialize the for_serde body (for put/post/patch etc)
pub struct SerdeJsonSerializer {}

#[cfg(feature = "for_serde")]
#[derive(Debug, Clone, Copy)]
// SerdeJsonSerializerForBytes Serialize the for_serde body (for put/post/patch etc)
pub struct SerdeJsonSerializerForBytes {}
#[cfg(feature = "for_serde")]
impl<T: Serialize> BodySerializer<T, Bytes> for SerdeJsonSerializerForBytes {
    fn encode(&self, origin: T) -> StdResult<Bytes, Box<dyn StdError>> {
        let serialized = serde_json::to_vec(&origin)?;

        Ok(Bytes::from(serialized))
    }
}
#[cfg(feature = "for_serde")]
pub const DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES: SerdeJsonSerializerForBytes =
    SerdeJsonSerializerForBytes {};

#[cfg(feature = "for_serde")]
impl<T: Serialize, B> BodySerializer<T, B> for SerdeJsonSerializer
where
    B: From<Bytes>,
{
    fn encode(&self, origin: T) -> StdResult<B, Box<dyn StdError>> {
        let serialized = DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES.encode(origin)?;

        Ok(B::from(serialized))
    }
}
#[cfg(feature = "for_serde")]
pub const DEFAULT_SERDE_JSON_SERIALIZER: SerdeJsonSerializer = SerdeJsonSerializer {};

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
pub const DEFAULT_SERDE_JSON_DESERIALIZER: SerdeJsonDeserializer = SerdeJsonDeserializer {};

pub trait BaseAPI<Client, Req, Res, Method, Header, B> {
    fn set_base_url(&mut self, url: Url);
    fn get_base_url(&self) -> Url;
    fn set_default_header(&mut self, header: Option<Header>);
    fn get_default_header(&self) -> Option<Header>;

    fn get_simple_http(&mut self) -> &mut SimpleHTTP<Client, Req, Res, Method, Header, B>;
}

pub trait BaseService<Client, Req, Res, Method, Header, B> {
    fn get_simple_api(&self) -> &Arc<Mutex<dyn BaseAPI<Client, Req, Res, Method, Header, B>>>;
    fn _call_common(
        &self,
        method: Method,
        header: Option<Header>,
        relative_url: String,
        content_type: String,
        path_param: Option<PathParam>,
        query_param: Option<QueryParam>,
        body: B,
    ) -> Pin<Box<dyn Future<Output = StdResult<Box<B>, Box<dyn StdError>>>>>;

    fn body_to_bytes(
        &self,
        body: B,
    ) -> Pin<Box<dyn Future<Output = StdResult<Bytes, Box<dyn StdError + Send + Sync>>>>>;
}

impl<Client, Req, Res, Method, Header, B> dyn BaseService<Client, Req, Res, Method, Header, B> {
    pub fn set_base_url(&self, url: Url) {
        self.get_simple_api().lock().unwrap().set_base_url(url);
    }
    pub fn get_base_url(&self) -> Url {
        self.get_simple_api().lock().unwrap().get_base_url()
    }
    pub fn set_default_header(&self, header: Option<Header>) {
        self.get_simple_api()
            .lock()
            .unwrap()
            .set_default_header(header);
    }
    pub fn get_default_header(&self) -> Option<Header> {
        self.get_simple_api().lock().unwrap().get_default_header()
    }
    pub fn set_client(
        &self,
        client: Arc<Mutex<dyn BaseClient<Client, Req, Res, Method, Header, B>>>,
    ) {
        self.get_simple_api()
            .lock()
            .unwrap()
            .get_simple_http()
            .set_client(client);
    }
    pub fn set_timeout_millisecond(&self, timeout_millisecond: u64) {
        self.get_simple_api()
            .lock()
            .unwrap()
            .get_simple_http()
            .timeout_millisecond = timeout_millisecond;
    }
    pub fn get_timeout_millisecond(&self) -> u64 {
        self.get_simple_api()
            .lock()
            .unwrap()
            .get_simple_http()
            .timeout_millisecond
    }

    pub fn add_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.get_simple_api()
            .lock()
            .unwrap()
            .get_simple_http()
            .add_interceptor(interceptor);
    }
    pub fn add_interceptor_front(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.get_simple_api()
            .lock()
            .unwrap()
            .get_simple_http()
            .add_interceptor_front(interceptor);
    }
    pub fn delete_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.get_simple_api()
            .lock()
            .unwrap()
            .get_simple_http()
            .delete_interceptor(interceptor);
    }
}

impl<Client, Req, Res, Method, Header, B> dyn BaseService<Client, Req, Res, Method, Header, B>
where
    Req: 'static,
{
    pub fn add_interceptor_fn(
        &mut self,
        func: impl FnMut(&mut Req) -> StdResult<(), Box<dyn StdError>> + Send + Sync + 'static,
    ) -> Arc<InterceptorFunc<Req>> {
        self.get_simple_api()
            .lock()
            .unwrap()
            .get_simple_http()
            .add_interceptor_fn(func)
    }
}

impl<Client, Req, Res, Method, Header, B> dyn BaseService<Client, Req, Res, Method, Header, B> {
    pub fn make_api_response_only<R>(
        &self,
        base: Arc<dyn BaseService<Client, Req, Res, Method, Header, B>>,
        method: Method,
        relative_url: impl Into<String>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIResponseOnly<R, Client, Req, Res, Method, Header, B> {
        APIResponseOnly {
            0: self.make_api_no_body(
                base,
                method,
                relative_url,
                response_deserializer,
                _return_type,
            ),
        }
    }
    pub fn make_api_no_body<R>(
        &self,
        base: Arc<dyn BaseService<Client, Req, Res, Method, Header, B>>,
        method: Method,
        relative_url: impl Into<String>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APINoBody<R, Client, Req, Res, Method, Header, B> {
        APINoBody {
            base,
            method,
            relative_url: relative_url.into(),
            response_deserializer,
            content_type: "".to_string(),
        }
    }
    pub fn make_api_has_body<T, R>(
        &self,
        base: Arc<dyn BaseService<Client, Req, Res, Method, Header, B>>,
        method: Method,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        request_serializer: Arc<dyn BodySerializer<T, B>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIHasBody<T, R, Client, Req, Res, Method, Header, B> {
        APIHasBody {
            base,
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
        base: Arc<dyn BaseService<Client, Req, Res, Method, Header, B>>,
        method: Method,
        relative_url: impl Into<String>,
        // request_serializer: Arc<dyn BodySerializer<FormData, (String, B)>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIMultipart<FormData, R, Client, Req, Res, Method, Header, B>
    where
        B: From<Bytes>,
    {
        APIMultipart {
            base,
            method,
            relative_url: relative_url.into(),
            request_serializer: Arc::new(DEFAULT_MULTIPART_SERIALIZER),
            response_deserializer,
        }
    }
}

// APIResponseOnly API with only response options
// R: Response body Type
pub struct APIResponseOnly<R, Client, Req, Res, Method, Header, B>(
    APINoBody<R, Client, Req, Res, Method, Header, B>,
);
impl<R, Client, Req, Res, Method, Header, B>
    APIResponseOnly<R, Client, Req, Res, Method, Header, B>
{
    pub async fn call(&self) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        self.call_with_options(None, None::<QueryParam>).await
    }
    pub async fn call_with_options(
        &self,
        header: Option<Header>,
        query_param: Option<impl Into<QueryParam>>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        self.0
            .call_with_options(header, None::<PathParam>, query_param)
            .await
    }
}

// APINoBody API without request body options
// R: Response body Type
pub struct APINoBody<R, Client, Req, Res, Method, Header, B> {
    base: Arc<dyn BaseService<Client, Req, Res, Method, Header, B>>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<R, Client, Req, Res, Method, Header, B> APINoBody<R, Client, Req, Res, Method, Header, B> {
    pub async fn call(&self, path_param: Option<PathParam>) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        self.call_with_options(None, path_param, None::<QueryParam>)
            .await
    }

    pub async fn call_with_options(
        &self,
        header: Option<Header>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        let body = self
            .base
            ._call_common(
                self.method.clone(),
                header,
                self.relative_url.clone(),
                self.content_type.clone(),
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
                B::default(),
            )
            .await?;
        // let mut target = Box::new(target);
        // let body = Box::new(body);
        // let bytes = hyper::body::to_bytes(*body).await?;
        let result = self.base.body_to_bytes(*body).await;
        if result.is_err() {
            return Err(result.err().unwrap());
        }
        let bytes = result.ok().unwrap();
        let target = self.response_deserializer.decode(&bytes)?;

        Ok(target)
    }
}

// APIHasBody API with request body options
// T: Request body Type
// R: Response body Type
pub struct APIHasBody<T, R, Client, Req, Res, Method, Header, B> {
    base: Arc<dyn BaseService<Client, Req, Res, Method, Header, B>>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub request_serializer: Arc<dyn BodySerializer<T, B>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, Client, Req, Res, Method, Header, B>
    APIHasBody<T, R, Client, Req, Res, Method, Header, B>
{
    pub async fn call(
        &self,
        path_param: Option<impl Into<PathParam>>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        self.call_with_options(None, path_param, None::<QueryParam>, sent_body)
            .await
    }

    pub async fn call_with_options(
        &self,
        header: Option<Header>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        // let mut sent_body = Box::new(sent_body);
        let body = self
            .base
            ._call_common(
                self.method.clone(),
                header,
                self.relative_url.clone(),
                self.content_type.clone(),
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
                self.request_serializer.encode(sent_body)?,
            )
            .await?;

        // let mut target = Box::new(target);
        // let body = Box::new(body);
        // let bytes = hyper::body::to_bytes(*body).await?;
        let result = self.base.body_to_bytes(*body).await;
        if result.is_err() {
            return Err(result.err().unwrap());
        }
        let bytes = result.ok().unwrap();
        let target = self.response_deserializer.decode(&bytes)?;

        Ok(target)
    }
}

// APIMultipart API with request body options
// T: Request body Type(multipart)
// R: Response body Type
pub struct APIMultipart<T, R, Client, Req, Res, Method, Header, B> {
    pub base: Arc<dyn BaseService<Client, Req, Res, Method, Header, B>>,
    pub method: Method,
    pub relative_url: String,
    // pub content_type: String,
    pub request_serializer: Arc<dyn BodySerializer<T, (String, B)>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, Client, Req, Res, Method, Header, B>
    APIMultipart<T, R, Client, Req, Res, Method, Header, B>
{
    pub async fn call(
        &self,
        path_param: Option<impl Into<PathParam>>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        self.call_with_options(None, path_param, None::<QueryParam>, sent_body)
            .await
    }

    pub async fn call_with_options(
        &self,
        header: Option<Header>,
        path_param: Option<impl Into<PathParam>>,
        query_param: Option<impl Into<QueryParam>>,
        sent_body: T,
    ) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
        Method: Clone,
    {
        // let mut sent_body = Box::new(sent_body);
        let (content_type_with_boundary, sent_body) = self.request_serializer.encode(sent_body)?;
        let body = self
            .base
            ._call_common(
                self.method.clone(),
                header,
                self.relative_url.clone(),
                content_type_with_boundary,
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
                sent_body,
            )
            .await?;

        // let mut target = Box::new(target);
        // let body = Box::new(body);
        // let bytes = hyper::body::to_bytes(*body).await?;
        let result = self.base.body_to_bytes(*body).await;
        if result.is_err() {
            return Err(result.err().unwrap());
        }
        let bytes = result.ok().unwrap();
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

// SimpleAPI SimpleAPI inspired by Retrofits
pub struct SimpleAPI<Client, Req, Res, Method, Header, B> {
    pub simple_http: SimpleHTTP<Client, Req, Res, Method, Header, B>,
    pub base_url: Url,
    pub default_header: Option<Header>,
}

impl<Client, Req, Res, Method, Header: Default, B> SimpleAPI<Client, Req, Res, Method, Header, B> {
    pub fn new_with_options(
        simple_http: SimpleHTTP<Client, Req, Res, Method, Header, B>,
        base_url: Url,
    ) -> Self {
        SimpleAPI {
            simple_http,
            base_url,
            default_header: None,
        }
    }
}

// #[inline]
// #[derive(Debug, Clone)]
