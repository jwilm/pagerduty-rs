//! Integration API
//!
//! The PagerDuty event integration API is how you would add PagerDuty's advanced alerting
//! functionality to any system that can make an HTTP API call. You can now add phone, SMS and email
//! alerting to your monitoring tools, ticketing systems and custom software.
//!
//! # Description
//!
//! The API was designed to allow you to easily integrate a monitoring system with a Service in
//! PagerDuty. Monitoring systems generally send out events when problems are detected and when
//! these problems have been resolved (fixed). Some more advanced systems also understand the
//! concept of acknowledgements: problems can be acknowledged by an engineer to signal he or she is
//! working on fixing the issue.
//!
//! Since monitoring systems emit events, the API is based around accepting events. Incoming events
//! (sent via the API) are routed to a PagerDuty service and processed. They may result in a new
//! incident being created, or an existing incident being acknowledged or resolved.
//!
//! The same event-based API can also be used to integrate a PagerDuty service with ticketing
//! systems and various other software tools.
//!
//! # API Limits
//!
//! There is a limit on the number of events that a service can accept at any given time. Depending
//! on the behavior of the incoming traffic and how many incidents are being created at once, we
//! reduce our throttle dynamically.
//!
//! If each of the events your monitoring system is sending is important, be sure to retry on a 403
//! response code, preferably with a back off.
//!
//! # Response codes and Retry Logic
//!
//! Ideally, the API request will succeed and the PagerDuty server will indicate that it
//! successfully received that event. In practice, the request may fail due to various reasons.
//!
//! The following table shows the possible results of the API request and if you need to retry the
//! API call for that result:
//!
//! | Result           | Description                                                                                   | Retry?                       |
//! |------------------|-----------------------------------------------------------------------------------------------|------------------------------|
//! | 200              | OK - The event has been accepted by PagerDuty. See below for details.                         | No                           |
//! | 400              | Bad Request - Check that the JSON is valid. See below for details.                            | No                           |
//! | 403              | Forbidden - Too many API calls at a time.                                                     | Yes - retry after some time. |
//! | 5xx              | Internal Server Error - the PagerDuty server experienced an error while processing the event. | Yes - retry after some time. |
//! | Networking Error | Error while trying to communicate with PagerDuty servers.                                     | Yes - retry after some time. |
//!

use std::borrow::Cow;

use hyper::header::Headers;
use hyper::method::Method;
use hyper::status::StatusCode;

use serde::Serialize;
use serde_json::{from_str, to_string, to_value, Value as Json};

use AuthToken;
use request::{self, Requestable};

/// Event to report a new or ongoing problem.
///
/// When PagerDuty receives a trigger event, it will either open a new incident, or add
/// a new trigger log entry to an existing incident, depending on the provided incident_key.
#[derive(Debug, Serialize)]
pub struct TriggerEvent<'a> {
    service_key: Cow<'a, str>,

    event_type: &'static str,

    description: Cow<'a, str>,

    #[serde(skip_serializing_if="Option::is_none")]
    incident_key: Option<Cow<'a, str>>,

    #[serde(skip_serializing_if="Option::is_none")]
    client: Option<Cow<'a, str>>,

    #[serde(skip_serializing_if="Option::is_none")]
    client_url: Option<Cow<'a, str>>,

    #[serde(skip_serializing_if="Option::is_none")]
    details: Option<Json>,

    #[serde(skip_serializing_if="Vec::is_empty")]
    contexts: Vec<Context<'a>>,
}

impl<'a> TriggerEvent<'a> {
    /// Create a new trigger event payload
    ///
    /// service_key: The GUID of one of your "Generic API" services. This is the "service key"
    /// listed on a Generic API's service detail page.
    ///
    /// description: A short description of the problem that led to this trigger. This field (or a
    /// truncated version) will be used when generating phone calls, SMS messages and alert emails.
    /// It will also appear on the incidents tables in the PagerDuty UI. The maximum length is 1024
    /// characters.
    pub fn new<S>(service_key: S, description: S) -> Self
        where S: Into<Cow<'a, str>>
    {
        TriggerEvent {
            service_key: service_key.into(),
            event_type: "trigger",
            description: description.into(),
            incident_key: None,
            client: None,
            client_url: None,
            details: None,
            contexts: Vec::new(),
        }
    }

    /// Set incident_key
    ///
    /// Identifies the incident to which this trigger event should be applied. If there's no open
    /// (i.e. unresolved) incident with this key, a new one will be created. If there's already an
    /// open incident with a matching key, this event will be appended to that incident's log. The
    /// event key provides an easy way to "de-dup" problem reports.
    pub fn set_incident_key<S>(mut self, incident_key: S) -> Self
        where S: Into<Cow<'a, str>>
    {
        self.incident_key = Some(incident_key.into());
        self
    }

    /// Set event's client
    ///
    /// The name of the monitoring client that is triggering this event.
    pub fn set_client<S>(mut self, client: S) -> Self
        where S: Into<Cow<'a, str>>
    {
        self.client = Some(client.into());
        self
    }

    /// Set event's client_url
    ///
    /// The URL of the monitoring client that is triggering this event.
    pub fn set_client_url<S>(mut self, client_url: S) -> Self
        where S: Into<Cow<'a, str>>
    {
        self.client_url = Some(client_url.into());
        self
    }

    /// Set event details
    ///
    /// An arbitrary JSON object containing any data you'd like included in the incident log.
    ///
    /// # Examples
    /// ```no_run
    /// # #![feature(custom_derive, plugin)]
    /// # #![plugin(serde_macros)]
    /// #
    /// # extern crate serde;
    /// # extern crate pagerduty;
    /// #
    /// # use pagerduty::integration::TriggerEvent;
    /// // Extra data to be included with the event. Anything that implements
    /// // Serialize can be passed to `set_details`.
    /// #[derive(Serialize)]
    /// struct Details {
    ///     what: &'static str,
    ///     count: i32,
    /// }
    ///
    /// # fn main() {
    /// // Create a trigger event and include custom data
    /// TriggerEvent::new("service_key", "event description")
    ///     .set_details(&Details {
    ///          what: "Server fire",
    ///          count: 1,
    ///     });
    /// # }
    ///
    /// ```
    pub fn set_details<T: ?Sized>(mut self, details: &T) -> Self
        where T: Serialize
    {
        self.details = Some(to_value(details));
        self
    }

    /// Add a Context to this event
    ///
    /// Contexts to be included with the incident trigger such as links to graphs or images. A
    /// "type" is required for each context submitted. For type "link", an "href" is required. You
    /// may optionally specify "text" with more information about the link. For type "image", "src"
    /// must be specified with the image src. You may optionally specify an "href" or an "alt" with
    /// this image.
    pub fn add_context(mut self, context: Context<'a>) -> Self {
        self.contexts.push(context);
        self
    }
}

/// An informational asset attached to the incident
///
/// This Context type is really a union of two different types, Image and Link. Due to object safety
/// issues, it's not possible to have a Context trait that can be serialized with Serde.
///
/// In the case that Context is an image, it must have a `src` attribute and may optionally have an
/// `href` and `alt` attributes. In the case of a link, context must have `href` and may optionally
/// include `text. To enforce these invariants, all of the fields are kept private, and all of the
/// properties must be specifed at once using the `link` and `image` methods.
#[derive(Debug, Serialize)]
pub struct Context<'a> {
    /// The type of context being attached to the incident. This will be a "link" or "image".
    #[serde(rename = "type")]
    context_type: &'static str,

    /// The source of the image being attached to the incident. This image must be served via HTTPS.
    #[serde(skip_serializing_if="Option::is_none")]
    src: Option<Cow<'a, str>>,

    /// Optional link for the image OR The link being attached to the incident.
    #[serde(skip_serializing_if="Option::is_none")]
    href: Option<Cow<'a, str>>,

    /// Optional alternative text for the image.
    #[serde(skip_serializing_if="Option::is_none")]
    alt: Option<Cow<'a, str>>,

    /// Optional information pertaining to the incident.
    #[serde(skip_serializing_if="Option::is_none")]
    text: Option<Cow<'a, str>>,
}

impl<'a> Context<'a> {
    /// Create a `link` context object
    pub fn link<S>(href: S, text: S) -> Context<'a>
        where S: Into<Cow<'a, str>>
    {
        Context {
            context_type: "link",
            href: Some(href.into()),
            text: Some(text.into()),
            alt: None,
            src: None,
        }
    }

    /// Create an `image` context object
    pub fn image<S>(src: S, href: Option<S>, alt: Option<S>) -> Context<'a>
        where S: Into<Cow<'a, str>>
    {
        Context {
            context_type: "image",
            src: Some(src.into()),
            href: href.map(|s| s.into()),
            alt: alt.map(|s| s.into()),
            text: None,
        }
    }
}

macro_rules! shared_event_type {
    { $(#[$attr:meta])* name => $name:ident; event_type => $event_type:expr } => {

        $(#[$attr])*
        #[derive(Debug, Serialize)]
        pub struct $name<'a> {
            service_key: Cow<'a, str>,
            event_type: &'static str,
            incident_key: Cow<'a, str>,

            #[serde(skip_serializing_if="Option::is_none")]
            description: Option<Cow<'a, str>>,

            #[serde(skip_serializing_if="Option::is_none")]
            details: Option<Json>,
        }

        impl<'a> $name<'a> {
            /// Create a new event
            ///
            /// * **service_key**: The GUID of one of your "Events API" services. This is the
            /// "service key" listed on a Generic API's service detail page.
            ///
            /// * **incident_key**: Identifies the incident to resolve. This should be the
            /// `incident_key` you received back when the incident was first opened by a trigger
            /// event. Resolve events referencing resolved or nonexistent incidents will be
            /// discarded.
            pub fn new<S>(service_key: S, incident_key: S) -> Self
                where S: Into<Cow<'a, str>>
            {
                $name {
                    service_key: service_key.into(),
                    event_type: $event_type,
                    incident_key: incident_key.into(),
                    description: None,
                    details: None,
                }
            }

            /// Set event details
            ///
            /// An arbitrary JSON object containing any data you'd like included in the incident
            /// log.
            ///
            /// For an example, please see the similar
            /// [`TriggerEvent::set_details`](struct.TriggerEvent.html#method.set_details).
            pub fn set_details<T: ?Sized>(mut self, details: &T) -> Self
                where T: Serialize
            {
                self.details = Some(to_value(details));
                self
            }

            /// Set text that will appear in the incident's log associated with this event.
            pub fn set_description<S>(mut self, description: S) -> Self
                where S: Into<Cow<'a, str>>
            {
                self.description = Some(description.into());
                self
            }
        }

        impl<'a> Requestable for $name<'a> {
            type Response = Response;

            fn body(&self) -> String {
                to_string(&self).unwrap()
            }

            fn method(&self) -> Method {
                Method::Post
            }

            fn get_response(status: StatusCode,
                            headers: &Headers,
                            body: &str) -> request::Result<Response> {
                Response::get_response(status, headers, body)
            }
        }


    }
}

shared_event_type! {
    /// Cause the referenced incident to enter the resolved state.
    ///
    /// Once an incident is resolved, it won't generate any additional notifications. New trigger
    /// events with the same incident_key as a resolved incident won't re-open the incident.
    /// Instead, a new incident will be created. Your monitoring tools should send PagerDuty a
    /// resolve event when the problem that caused the initial trigger event has been fixed.
    name => ResolveEvent; event_type => "resolve"
}

shared_event_type! {
    /// Acknowledge events cause the referenced incident to enter the acknowledged state.
    ///
    /// While an incident is acknowledged, it won't generate any additional notifications, even if
    /// it receives new trigger events. Your monitoring tools should send PagerDuty an acknowledge
    /// event when they know someone is presently working on the problem.
    name => AcknowledgeEvent; event_type => "acknowledge"
}

/// Response types from the integration API
pub mod response {
    /// If the request is invalid, PagerDuty will respond with HTTP code 400 and this object
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    pub struct BadRequest {
        /// invalid event
        pub status: String,

        /// A description of the problem
        pub message: String,

        /// An array of specific error messages
        pub errors: Vec<String>,
    }

    /// If the request is well-formatted, PagerDuty will respond with HTTP code 200 and this object.
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    pub struct Success {
        /// The string _"success"_
        pub status: String,

        /// Event processed
        pub message: String,

        /// The key of the incident that will be affected by the request.
        pub incident_key: String,
    }
}

/// A Response from the integration API
///
/// A union of all possible responses for the integration API.
#[derive(Debug, PartialEq, Eq)]
pub enum Response {
    Success(response::Success),
    BadRequest(response::BadRequest),
    Forbidden,
    InternalServerError,
}

impl Response {
    fn get_response(status: StatusCode,
                    _headers: &Headers,
                    body: &str) -> request::Result<Response> {
        match status {
            StatusCode::Ok => {
                let res: response::Success = try!(from_str(body));
                Ok(Response::Success(res))
            },
            StatusCode::BadRequest => {
                let res: response::BadRequest = try!(from_str(body));
                Ok(Response::BadRequest(res))
            },
            StatusCode::Forbidden => {
                Ok(Response::Forbidden)
            },
            _ => {
                if status.is_server_error() {
                    Ok(Response::InternalServerError)
                } else {
                    Err(request::Error::UnexpectedApiResponse)
                }
            }
        }
    }
}

impl<'a> Requestable for TriggerEvent<'a> {
    type Response = Response;

    fn body(&self) -> String {
        to_string(&self).unwrap()
    }

    fn method(&self) -> Method {
        Method::Post
    }

    fn get_response(status: StatusCode,
                    headers: &Headers,
                    body: &str) -> request::Result<Response> {
        Response::get_response(status, headers, body)
    }
}


/// Send a TriggerEvent request
pub fn trigger(auth: &AuthToken, event: &TriggerEvent) -> request::Result<Response> {
    request::perform(auth, event)
}

/// Send a ResolveEvent request
pub fn resolve(auth: &AuthToken, event: &ResolveEvent) -> request::Result<Response> {
    request::perform(auth, event)
}

/// Send an AcknowledgeEvent request
pub fn acknowledge(auth: &AuthToken, event: &AcknowledgeEvent) -> request::Result<Response> {
    request::perform(auth, event)
}

#[cfg(test)]
mod tests {
    use super::{TriggerEvent, Context};

    use serde_json::{from_str, to_string, Value as Json};

    #[test]
    fn context_to_json() {
        let expected: Json = from_str(stringify!({
            "type": "image",
            "src": "https://www.example.com"
        })).expect("expected is valid json");

        let context = Context::image("https://www.example.com", None, None);
        let json_string = to_string(&context).unwrap();
        let actual: Json = from_str(&json_string).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn trigger_event_to_json() {
        let expected: Json = from_str(stringify!({
            "event_type": "trigger",
            "service_key": "the service key",
            "description": "Houston, we have a problem"
        })).expect("expected is valid json");

        let event = TriggerEvent::new("the service key", "Houston, we have a problem");
        let json_string = to_string(&event).unwrap();
        let actual: Json = from_str(&json_string).unwrap();

        assert_eq!(actual, expected);
    }


    #[test]
    fn trigger_event_with_contexts_to_json() {
        #[derive(Debug, Serialize)]
        struct Details {
            last_delivery_time: i32,
        }

        let expected: Json = from_str(stringify!({
            "event_type": "trigger",
            "service_key": "the service key",
            "description": "Houston, we have a problem",
            "contexts": [
                {
                    "type": "image",
                    "src": "https://www.example.com"
                },
                {
                    "type": "link",
                    "href": "https://www.example.com",
                    "text": "a link"
                }
            ],
            "details": {
                "last_delivery_time": 10
            },
            "incident_key": "KEY123"
        })).expect("expected is valid json");

        let event = TriggerEvent::new("the service key", "Houston, we have a problem")
                        .set_incident_key("KEY123")
                        .set_details(&Details { last_delivery_time: 10 })
                        .add_context(Context::image("https://www.example.com", None, None))
                        .add_context(Context::link("https://www.example.com", "a link"));

        let json_string = to_string(&event).unwrap();
        let actual: Json = from_str(&json_string).unwrap();

        println!("{:?}", event);

        assert_eq!(actual, expected);
    }
}

#[cfg(feature = "live_tests")]
mod live_tests {
    use AuthToken;

    use super::{trigger, Response, TriggerEvent};

    #[test]
    fn invalid_auth_token_is_rejected() {
        let event = TriggerEvent::new("0123456789abcdef0123456789abcdef", "Test event");
        let token = AuthToken::new("abc");
        let response = trigger(&token, &event).unwrap();

        match response {
            Response::Success(_) => (),
            _ => panic!("Should have been success")
        }
    }
}
