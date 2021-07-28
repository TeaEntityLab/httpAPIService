use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::result::Result as StdResult;
use std::str::FromStr;

#[cfg(feature = "multipart")]
use formdata::FormData;
use futures::Future;
use http::method::Method;
use hyper::body::HttpBody;
use hyper::client::{connect::Connect, HttpConnector};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Body, HeaderMap, Request, Uri};
use url::Url;

use super::simple_http;
use super::simple_http::{SimpleHTTP, SimpleHTTPResponse};
// use simple_http;

// PathParam Path params for API usages
type PathParam = HashMap<String, Box<dyn Debug>>;

// APINoBody API without request body options
trait APINoBody<T>: FnMut(PathParam, &mut T) -> dyn Future<Output = SimpleHTTPResponse>
where
    Self: std::marker::Sized,
{
}

// APIHasBody API with request body options
trait APIHasBody<T, B = Body>:
    FnMut(PathParam, B, &mut T) -> dyn Future<Output = SimpleHTTPResponse>
where
    Self: std::marker::Sized,
{
}

// APIResponseOnly API with only response options
trait APIResponseOnly<T>: FnMut(&mut T) -> dyn Future<Output = SimpleHTTPResponse>
where
    Self: std::marker::Sized,
{
}

// BodySerializer Serialize the body (for put/post/patch etc)
trait BodySerializer<T, B = Body>: FnMut(&mut T) -> StdResult<B, Box<dyn StdError>>
where
    Self: std::marker::Sized,
{
}

// BodyDeserializer Deserialize the body (for response)
trait BodyDeserializer<T, B = Body>: FnMut(B, &mut T) -> StdResult<T, Box<dyn StdError>>
where
    Self: std::marker::Sized,
{
}

#[cfg(feature = "multipart")]
// MultipartSerializer Serialize the multipart body (for put/post/patch etc)
type MultipartSerializer<B = Body> = dyn FnMut(&mut FormData) -> StdResult<B, dyn StdError>;

// SimpleAPI SimpleAPI inspired by Retrofits
pub struct SimpleAPI<C, B = Body> {
    pub simple_http: SimpleHTTP<C, B>,
    pub base_url: Url,
    pub default_header: HeaderMap,
    // RequestSerializerForMultipart: MultipartSerializer,
    // RequestSerializerForJSON:      BodySerializer,
    // ResponseDeserializer:          BodyDeserializer,
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
        path_param: PathParam,
        body: B,
    ) -> StdResult<Request<B>, Box<dyn StdError>> {
        let mut req = Request::new(body);
        // Url
        match self.base_url.join(relative_url.as_str()) {
            Ok(url) => {
                let mut url = url.to_string();

                for (k, v) in path_param.into_iter() {
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
        path_param: PathParam,
        body: FormData,
    ) -> StdResult<Request<Body>, Box<dyn StdError>> {
        let (body, boundary) = simple_http::body_from_multipart(body)?;
        self.make_request(
            method,
            relative_url,
            // if content_type.is_empty() {
            simple_http::get_content_type_from_multipart_boundary(boundary)?,
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
