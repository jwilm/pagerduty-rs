use std::borrow::Cow;

/// A token used to authorize requests to PagerDuty.
///
/// The AuthToken is expected to be created with a String or &str passed to `AuthToken::new`. Since
/// AuthToken uses a Cow internally, no extra allocations occur.
///
/// # Example
///
/// ```
/// # use pagerduty::AuthToken;
/// let s = String::from("token");
/// // Only valid as long as the string slice is valid
/// let ref_token = AuthToken::new(&s[..]);
///
/// // Owned version may be desired in some cases
/// let owned_token = AuthToken::new(String::from("token"));
/// ```
pub struct AuthToken<'a>(Cow<'a, str>);

impl<'a> AuthToken<'a> {
    pub fn new<T>(raw_token: T) -> AuthToken<'a>
        where T: Into<Cow<'a, str>>
    {
        AuthToken(raw_token.into())
    }

    pub fn to_header(&self) -> ::hyper::header::Authorization<String> {
        ::hyper::header::Authorization(self.0.as_ref().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_auth_token_with_str_slice() {
        AuthToken::new("token");
    }

    #[test]
    fn make_auth_token_with_owned_string() {
        AuthToken::new(String::from("token"));
    }
}
