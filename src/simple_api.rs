/*!
In this module there're implementations & tests of `SimpleAPI`.
*/

use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::method::Method;
use url::Url;

pub use super::common::{
    BodyDeserializer, BodySerializer, PathParam, QueryParam, DEFAULT_DUMMY_BYPASS_DESERIALIZER,
    DEFAULT_DUMMY_BYPASS_SERIALIZER_FOR_BYTES,
};
use super::simple_http::{ClientCommon, Interceptor, InterceptorFunc, SimpleHTTP};

#[cfg(feature = "for_serde")]
pub use super::common::DEFAULT_SERDE_JSON_DESERIALIZER;

pub trait SimpleAPICommon<Client, Req, Res, Header, B> {
    fn set_base_url(&mut self, url: Url);
    fn get_base_url(&self) -> Url;
    fn set_default_header(&mut self, header: Option<Header>);
    fn get_default_header(&self) -> Option<Header>;

    fn get_simple_http(&mut self) -> &mut SimpleHTTP<Client, Req, Res, Header, B>;
}

pub trait BaseService<Client, Req, Res, Header, B> {
    fn get_simple_api(&self) -> &Arc<Mutex<dyn SimpleAPICommon<Client, Req, Res, Header, B>>>;
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

impl<Client, Req, Res, Header, B> dyn BaseService<Client, Req, Res, Header, B> {
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
    pub fn set_client(&self, client: Arc<dyn ClientCommon<Client, Req, Res, Header, B>>) {
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

impl<Client, Req, Res, Header, B> dyn BaseService<Client, Req, Res, Header, B>
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

impl<Client, Req, Res, Header, B> dyn BaseService<Client, Req, Res, Header, B> {
    pub fn make_api_response_only<R>(
        &self,
        base: Arc<dyn BaseService<Client, Req, Res, Header, B>>,
        method: Method,
        relative_url: impl Into<String>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIResponseOnly<R, Client, Req, Res, Header, B> {
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
        base: Arc<dyn BaseService<Client, Req, Res, Header, B>>,
        method: Method,
        relative_url: impl Into<String>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APINoBody<R, Client, Req, Res, Header, B> {
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
        base: Arc<dyn BaseService<Client, Req, Res, Header, B>>,
        method: Method,
        relative_url: impl Into<String>,
        content_type: impl Into<String>,
        request_serializer: Arc<dyn BodySerializer<T, B>>,
        response_deserializer: Arc<dyn BodyDeserializer<R>>,
        _return_type: &R,
    ) -> APIHasBody<T, R, Client, Req, Res, Header, B> {
        APIHasBody {
            base,
            method,
            relative_url: relative_url.into(),
            content_type: content_type.into(),
            request_serializer,
            response_deserializer,
        }
    }
}

// APIResponseOnly API with only response options
// R: Response body Type
pub struct APIResponseOnly<R, Client, Req, Res, Header, B>(
    APINoBody<R, Client, Req, Res, Header, B>,
);
impl<R, Client, Req, Res, Header, B> APIResponseOnly<R, Client, Req, Res, Header, B>
where
// C: Connect + Clone + Send + Sync + 'static,
// B: HttpBody + Send + 'static,
// B::Data: Send,
// B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(&self) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
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
    {
        self.0
            .call_with_options(header, None::<PathParam>, query_param)
            .await
    }
}

// APINoBody API without request body options
// R: Response body Type
pub struct APINoBody<R, Client, Req, Res, Header, B> {
    base: Arc<dyn BaseService<Client, Req, Res, Header, B>>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<R, Client, Req, Res, Header, B> APINoBody<R, Client, Req, Res, Header, B>
where
// C: Connect + Clone + Send + Sync + 'static,
// B: HttpBody + Send + 'static,
// B::Data: Send,
// B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn call(&self, path_param: Option<PathParam>) -> StdResult<Box<R>, Box<dyn StdError>>
    where
        B: Default,
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
pub struct APIHasBody<T, R, Client, Req, Res, Header, B> {
    base: Arc<dyn BaseService<Client, Req, Res, Header, B>>,
    pub method: Method,
    pub relative_url: String,
    pub content_type: String,

    pub request_serializer: Arc<dyn BodySerializer<T, B>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, Client, Req, Res, Header, B> APIHasBody<T, R, Client, Req, Res, Header, B>
where
// C: Connect + Clone + Send + Sync + 'static,
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
        B: Default,
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
                self.request_serializer.encode(&sent_body)?,
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
pub struct APIMultipart<T, R, Client, Req, Res, Header, B> {
    pub base: Arc<dyn BaseService<Client, Req, Res, Header, B>>,
    pub method: Method,
    pub relative_url: String,
    // pub content_type: String,
    pub request_serializer: Arc<dyn BodySerializer<T, (String, B)>>,
    pub response_deserializer: Arc<dyn BodyDeserializer<R>>,
}
impl<T, R, Client, Req, Res, Header, B> APIMultipart<T, R, Client, Req, Res, Header, B>
where
// C: Connect + Clone + Send + Sync + 'static,
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
        B: Default,
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
    {
        // let mut sent_body = Box::new(sent_body);
        let (content_type_with_boundary, sent_body) = self.request_serializer.encode(&sent_body)?;
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
pub struct SimpleAPI<Client, Req, Res, Header, B> {
    pub simple_http: SimpleHTTP<Client, Req, Res, Header, B>,
    pub base_url: Url,
    pub default_header: Option<Header>,
}

impl<Client, Req, Res, Header: Default, B> SimpleAPI<Client, Req, Res, Header, B> {
    pub fn new_with_options(
        simple_http: SimpleHTTP<Client, Req, Res, Header, B>,
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
