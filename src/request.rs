use std::borrow::Cow;
use std::io::{self, Read};

use hyper::header::{self, Headers};
use hyper::method::Method;
use hyper::status::StatusCode;
use hyper;

use serde_json;

use AuthToken;

/// Things that can be sent to the pagerduty API
pub trait Requestable {
    type Response;

    /// Headers for this request
    fn headers(&self) -> Headers {
        Headers::new()
    }

    /// URL for this request
    fn url<'a>(&'a self) -> Cow<'a, str> {
        "https://events.pagerduty.com/generic/2010-04-15/create_event.json".into()
    }

    /// Get the request body
    fn body(&self) -> String;

    /// Generate a response given a status, response headers, and response body.
    fn get_response(status: StatusCode,
                    headers: &Headers,
                    body: &str) -> Result<Self::Response>;

    /// HTTP Method for current request
    fn method(&self) -> Method;
}

/// Possible errors making an HTTP request and processing the response
#[derive(Debug)]
pub enum Error {
    /// Error from HTTP library; covers network errors as well
    Http(hyper::Error),

    /// Error deserializing a response from JSON
    Deserialize(serde_json::Error),

    /// Error reading response body from hyper response
    ReadResponse(io::Error),

    /// Unexpected API response
    ///
    /// The response parser is built to the PagerDuty API specification, so this shouldn't come up
    /// as long as their API doesn't device from the spec.
    UnexpectedApiResponse
}

impl ::std::error::Error for Error {
    fn cause(&self) -> Option<&::std::error::Error> {
        match *self {
            Error::Http(ref err) => Some(err),
            Error::Deserialize(ref err) => Some(err),
            Error::ReadResponse(ref err) => Some(err),
            Error::UnexpectedApiResponse => None,
        }
    }

    fn description(&self) -> &str {
        match *self {
            Error::Http(ref err) => err.description(),
            Error::Deserialize(ref err) => err.description(),
            Error::ReadResponse(ref err) => err.description(),
            Error::UnexpectedApiResponse => "Unexpected API response",
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            Error::Http(ref err) => {
                write!(f, "Error making HTTP request: {}", err)
            },
            Error::Deserialize(ref err) => {
                write!(f, "Error deserializing response as JSON: {}", err)
            },
            Error::ReadResponse(ref err) => {
                write!(f, "Error reading response body: {}", err)
            },
            Error::UnexpectedApiResponse => write!(f, "Unexpected API response"),
        }
    }
}

impl From<hyper::Error> for Error {
    fn from(val: hyper::Error) -> Error {
        Error::Http(val)
    }
}

impl From<serde_json::Error> for Error {
    fn from(val: serde_json::Error) -> Error {
        Error::Deserialize(val)
    }
}

impl From<io::Error> for Error {
    fn from(val: io::Error) -> Error {
        Error::ReadResponse(val)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub fn perform<R>(auth: &AuthToken, requestable: &R) -> Result<R::Response>
    where R: Requestable
{
    let client = hyper::Client::new();

    // Get request-specific body and headers
    let body = requestable.body();
    let mut headers = requestable.headers();

    // Add default headers
    headers.set(auth.to_header());
    headers.set(header::UserAgent("hyper/0.8.0 pagerduty-rs/0.1.0".into()));
    headers.set(header::ContentType::json());

    let mut res = try!(client.request(requestable.method(), requestable.url().as_ref())
        .headers(headers)
        .body(&body[..])
        .send());

    let mut response_body = String::new();
    try!(res.read_to_string(&mut response_body));

    Ok(try!(R::get_response(res.status, &res.headers, &response_body[..])))
}
