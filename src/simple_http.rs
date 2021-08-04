/*!
In this module there're implementations & tests of `SimpleHTTP`.
*/

use std::collections::{HashMap, VecDeque};
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};

use bytes::Bytes;

#[cfg(feature = "multipart")]
pub use super::common::generate_id;
#[cfg(feature = "multipart")]
use formdata::FormData;
#[cfg(feature = "multipart")]
use mime::MULTIPART_FORM_DATA;
#[cfg(feature = "multipart")]
use multer;
#[cfg(feature = "multipart")]
use multer::Multipart;

pub const DEFAULT_TIMEOUT_MILLISECOND: u64 = 30 * 1000;

/**
`Interceptor` defines an interface for intercepting through Requests.

# Arguments

* `B` - The generic type of request body data

# Remarks

It's the interface trait of Interceptor.
You could implement your own versions of interceptors

*/
pub trait Interceptor<R> {
    fn get_id(&self) -> String;
    fn intercept(&self, request: &mut R) -> StdResult<(), Box<dyn StdError>>;
}

/**
`InterceptorFunc` Implements an interceptor with a FnMut for intercepting through Requests.

# Arguments

* `B` - The generic type of request body data (default: `hyper::Body`)

# Remarks

It's a dummy implementations of Interceptor.
In most of Debugging/Observing cases it's useful enough.

*/
#[derive(Clone)]
pub struct InterceptorFunc<R> {
    id: String,
    func: Arc<Mutex<dyn FnMut(&mut R) -> StdResult<(), Box<dyn StdError>> + Send + Sync + 'static>>,
}
impl<R> InterceptorFunc<R> {
    /**
    Generate a new `InterceptorFunc` with the given `FnMut`.

    # Arguments

    * `func` - The given `FnMut`.

    */
    pub fn new<T>(func: T) -> InterceptorFunc<R>
    where
        T: FnMut(&mut R) -> StdResult<(), Box<dyn StdError>> + Send + Sync + 'static,
    {
        InterceptorFunc {
            id: Self::generate_id(),
            func: Arc::new(Mutex::new(func)),
        }
    }

    fn generate_id() -> String {
        generate_id()
    }
}
impl<R> Interceptor<R> for InterceptorFunc<R> {
    fn get_id(&self) -> String {
        return self.id.clone();
    }
    fn intercept(&self, request: &mut R) -> StdResult<(), Box<dyn StdError>> {
        let func = &mut *self.func.lock().unwrap();
        (func)(request)
    }
}

pub type SimpleHTTPResponse<R> = StdResult<R, Box<dyn StdError>>;

pub trait BaseClient<Client, Req, Res, Method, Header, B> {
    fn request(&self, req: Req) -> Pin<Box<dyn Future<Output = Res>>>;
}

/* SimpleHTTP SimpleHTTP inspired by Retrofits
*/
pub struct SimpleHTTP<Client, Req, Res, Method, Header, B> {
    pub client: Arc<dyn BaseClient<Client, Req, Res, Method, Header, B>>,
    pub interceptors: VecDeque<Arc<dyn Interceptor<Req>>>,
    pub timeout_millisecond: u64,
}

impl<Client, Req, Res, Method, Header, B> SimpleHTTP<Client, Req, Res, Method, Header, B> {
    pub fn new_with_options(
        client: Arc<dyn BaseClient<Client, Req, Res, Method, Header, B>>,
        interceptors: VecDeque<Arc<dyn Interceptor<Req>>>,
        timeout_millisecond: u64,
    ) -> Self {
        SimpleHTTP {
            client,
            interceptors,
            timeout_millisecond,
        }
    }

    pub fn set_client(&mut self, client: Arc<dyn BaseClient<Client, Req, Res, Method, Header, B>>) {
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

impl<Client, Req, Res, Method, Header, B> SimpleHTTP<Client, Req, Res, Method, Header, B>
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
pub fn get_content_type_from_multipart_boundary(
    boundary: Vec<u8>,
) -> StdResult<String, Box<dyn StdError>> {
    Ok(MULTIPART_FORM_DATA.to_string() + "; boundary=\"" + &String::from_utf8(boundary)? + "\"")
}
#[cfg(feature = "multipart")]
pub fn data_and_boundary_from_multipart(
    form_data: &FormData,
) -> StdResult<(Vec<u8>, Vec<u8>), Box<dyn StdError>> {
    let mut data = Vec::<u8>::new();
    let boundary = formdata::generate_boundary();
    formdata::write_formdata(&mut data, &boundary, form_data)?;

    Ok((data, boundary))
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
