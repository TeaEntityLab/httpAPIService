use std::collections::HashMap;
use std::error::Error as StdError;
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;

#[cfg(feature = "for_serde")]
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "multipart")]
use formdata::FormData;
#[cfg(feature = "multipart")]
use mime::MULTIPART_FORM_DATA;

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

/*
`PathParam` Path params for API usages
*/
pub type PathParam = HashMap<String, String>;
/*
`QueryParam` Query params for API usages
*/
pub type QueryParam = HashMap<String, String>;

#[macro_export]
macro_rules! path_param {
    ($( $key: expr => $val: expr ),*) => {{
         hyper_api_service::hash_map_string!(
             $( $key => $val )*
         )
    }}
}
#[macro_export]
macro_rules! query_param {
    ($( $key: expr => $val: expr ),*) => {{
         hyper_api_service::hash_map_string!(
             $( $key => $val )*
         )
    }}
}
#[macro_export]
macro_rules! hash_map_string {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = hyper_api_service::simple_api::PathParam::new();
         $( map.insert($key.into(), $val.into()); )*
         map
    }}
}

/*
`BodySerializer  Serialize the body (for put/post/patch etc)
*/
pub trait BodySerializer<T, B> {
    fn encode(&self, origin: &T) -> StdResult<B, Box<dyn StdError>>;
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
    fn encode(&self, origin: &Bytes) -> StdResult<Bytes, Box<dyn StdError>> {
        Ok(Bytes::from(origin.to_vec()))
    }
}
pub static DEFAULT_DUMMY_BYPASS_SERIALIZER_FOR_BYTES: DummyBypassSerializerForBytes =
    DummyBypassSerializerForBytes {};

#[cfg(feature = "multipart")]
#[derive(Debug, Clone, Copy)]
// MultipartSerializerForBytes Serialize the multipart body (for put/post/patch etc)
pub struct MultipartSerializerForBytes {}
#[cfg(feature = "multipart")]
impl BodySerializer<FormData, (String, Bytes)> for MultipartSerializerForBytes {
    fn encode(&self, origin: &FormData) -> StdResult<(String, Bytes), Box<dyn StdError>> {
        let (body, boundary) = data_and_boundary_from_multipart(origin)?;
        let content_type = get_content_type_from_multipart_boundary(boundary)?;

        Ok((content_type, Bytes::from(body)))
    }
}
#[cfg(feature = "multipart")]
pub static DEFAULT_MULTIPART_SERIALIZER_FOR_BYTES: MultipartSerializerForBytes =
    MultipartSerializerForBytes {};

#[cfg(feature = "for_serde")]
#[derive(Debug, Clone, Copy)]
// SerdeJsonSerializerForBytes Serialize the for_serde body (for put/post/patch etc)
pub struct SerdeJsonSerializerForBytes {}
#[cfg(feature = "for_serde")]
impl<T: Serialize> BodySerializer<T, Bytes> for SerdeJsonSerializerForBytes {
    fn encode(&self, origin: &T) -> StdResult<Bytes, Box<dyn StdError>> {
        let serialized = serde_json::to_vec(origin)?;

        Ok(Bytes::from(serialized))
    }
}
#[cfg(feature = "for_serde")]
pub static DEFAULT_SERDE_JSON_SERIALIZER_FOR_BYTES: SerdeJsonSerializerForBytes =
    SerdeJsonSerializerForBytes {};

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
pub static DEFAULT_DUMMY_BYPASS_DESERIALIZER: DummyBypassDeserializer = DummyBypassDeserializer {};

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
pub static DEFAULT_SERDE_JSON_DESERIALIZER: SerdeJsonDeserializer = SerdeJsonDeserializer {};

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

pub fn generate_id() -> String {
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    format!("{:?}{:?}", thread::current().id(), since_the_epoch)
}
