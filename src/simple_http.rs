/*!
In this module there're implementations & tests of `SimpleHTTP`.
*/

use std::collections::{HashMap, VecDeque};
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::Arc;

pub use super::common::{Interceptor, InterceptorFunc};
use bytes::Bytes;

#[cfg(feature = "multipart")]
pub use super::common::generate_id;
#[cfg(feature = "multipart")]
use multer;
#[cfg(feature = "multipart")]
use multer::Multipart;

pub const DEFAULT_TIMEOUT_MILLISECOND: u64 = 30 * 1000;

pub type SimpleHTTPResponse<R> = StdResult<R, Box<dyn StdError>>;

pub trait ClientCommon<Client, Req, Res, Header, B> {
    fn request(&self, req: Req) -> Pin<Box<dyn Future<Output = Res>>>;
}

/* SimpleHTTP SimpleHTTP inspired by Retrofits
*/
pub struct SimpleHTTP<Client, Req, Res, Header, B> {
    pub client: Arc<dyn ClientCommon<Client, Req, Res, Header, B>>,
    pub interceptors: VecDeque<Arc<dyn Interceptor<Req>>>,
    pub timeout_millisecond: u64,
}

impl<Client, Req, Res, Header, B> SimpleHTTP<Client, Req, Res, Header, B> {
    pub fn new_with_options(
        client: Arc<dyn ClientCommon<Client, Req, Res, Header, B>>,
        interceptors: VecDeque<Arc<dyn Interceptor<Req>>>,
        timeout_millisecond: u64,
    ) -> Self {
        SimpleHTTP {
            client,
            interceptors,
            timeout_millisecond,
        }
    }

    pub fn set_client(&mut self, client: Arc<dyn ClientCommon<Client, Req, Res, Header, B>>) {
        self.client = client;
    }

    pub fn add_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.interceptors.push_back(interceptor);
    }
    pub fn add_interceptor_front(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        self.interceptors.push_front(interceptor);
    }
    pub fn delete_interceptor(&mut self, interceptor: Arc<dyn Interceptor<Req>>) {
        let id;
        {
            id = interceptor.get_id();
        }

        for (index, obs) in self.interceptors.clone().iter().enumerate() {
            if obs.get_id() == id {
                // println!("delete_interceptor({});", interceptor);
                self.interceptors.remove(index);
                return;
            }
        }
    }
}

impl<Client, Req, Res, Header, B> SimpleHTTP<Client, Req, Res, Header, B>
where
    Req: 'static,
{
    pub fn add_interceptor_fn(
        &mut self,
        func: impl FnMut(&mut Req) -> StdResult<(), Box<dyn StdError>> + Send + Sync + 'static,
    ) -> Arc<InterceptorFunc<Req>> {
        let interceptor = Arc::new(InterceptorFunc::new(func));
        self.add_interceptor(interceptor.clone());

        interceptor
    }
}

#[cfg(feature = "multipart")]
#[derive(Debug)]
pub struct FormDataParseError {
    details: String,
}
#[cfg(feature = "multipart")]
impl StdError for FormDataParseError {}
#[cfg(feature = "multipart")]
impl FormDataParseError {
    pub fn new(msg: impl Into<String>) -> FormDataParseError {
        FormDataParseError {
            details: msg.into(),
        }
    }
}
#[cfg(feature = "multipart")]
impl std::fmt::Display for FormDataParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.details)
    }
}

#[cfg(feature = "multipart")]
pub async fn multer_multipart_to_hash_map(
    multipart: &mut Multipart<'_>,
) -> StdResult<HashMap<String, (String, String, Bytes)>, Box<dyn StdError>> {
    let mut result = HashMap::new();

    while let Some(field) = multipart.next_field().await? {
        let name = match field.name() {
            Some(s) => s.to_string(),
            None => {
                // Return error
                "".to_string()
            }
        };

        let file_name = match field.file_name() {
            Some(s) => s.to_string(),
            None => "".to_string(),
        };
        let data = if file_name.is_empty() {
            field.bytes().await?
        } else {
            Bytes::new()
        };

        result.insert(name.to_string(), (name, file_name, data));
    }

    Ok(result)
}

// #[inline]
// #[derive(Debug, Clone)]
