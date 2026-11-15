use std::fmt;

use anyhow::{Result, anyhow, bail};
use model::{Failure, FailureKind};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::transport::{Request, Transport};

const BASE: &str = "https://api.wemod.com";

pub(super) async fn call<T: DeserializeOwned>(
    t: &Transport,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    form: Option<&[(&str, &str)]>,
    expect: u16,
) -> Result<T> {
    let res = t.fetch(request(method, path, bearer, form)).await?;
    parse(res.status, &res.body, expect)
}

fn request(method: &str, path: &str, bearer: Option<&str>, form: Option<&[(&str, &str)]>) -> Request {
    let mut headers = vec![("Accept".to_owned(), "application/json".to_owned())];
    if let Some(token) = bearer {
        headers.push(("Authorization".to_owned(), format!("Bearer {token}")));
    }
    let body = form.map(|pairs| {
        headers.push(("Content-Type".to_owned(), "application/x-www-form-urlencoded".to_owned()));
        form_urlencoded::Serializer::new(String::new()).extend_pairs(pairs).finish()
    });
    Request { method: method.to_owned(), url: format!("{BASE}{path}"), headers, body }
}

// Never put the body in an error: it can hold tokens. Only the status and the JSON `error` field.
#[derive(Debug)]
struct HttpError {
    status: u16,
    error: Option<String>,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP {}", self.status)?;
        self.error.as_ref().map_or(Ok(()), |e| write!(f, ": {e}"))
    }
}

impl std::error::Error for HttpError {}

/// The server refused the credentials (OAuth `invalid_grant` is 400), as opposed to a network or server failure.
pub(super) fn is_rejected(e: &anyhow::Error) -> bool {
    e.downcast_ref::<HttpError>().is_some_and(|h| matches!(h.status, 400 | 401))
}

fn parse<T: DeserializeOwned>(status: u16, body: &str, expect: u16) -> Result<T> {
    if !(200..300).contains(&status) {
        let error = serde_json::from_str::<Value>(body).ok().and_then(|v| v["error"].as_str().map(str::to_owned));
        let kind = match status {
            401 | 403 => FailureKind::SignedOut,
            404 => FailureKind::NotFound,
            _ => FailureKind::Internal,
        };
        return Err(anyhow::Error::new(HttpError { status, error }).context(Failure::new(kind)));
    }
    if status != expect {
        bail!("unexpected HTTP status {status}");
    }
    serde_json::from_str(body).map_err(|_| anyhow!("unexpected response format"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_has_status_and_error_field_only() {
        let err = parse::<Value>(401, r#"{"error":"invalid_grant","access_token":"S3CRET"}"#, 200).unwrap_err();
        assert_eq!(err.root_cause().to_string(), "HTTP 401: invalid_grant");
        assert!(is_rejected(&err));
        let err = parse::<Value>(502, "<html>S3CRET</html>", 200).unwrap_err();
        assert_eq!(err.root_cause().to_string(), "HTTP 502");
        assert!(!is_rejected(&err));
    }

    #[test]
    fn unexpected_status_and_bad_json_never_include_the_body() {
        let err = parse::<Value>(200, r#"{"code":"S3CRET"}"#, 201).unwrap_err();
        assert_eq!(err.to_string(), "unexpected HTTP status 200");
        let err = parse::<Value>(200, "S3CRET", 200).unwrap_err();
        assert_eq!(err.to_string(), "unexpected response format");
    }

    #[test]
    fn request_has_accept_bearer_and_form() {
        let req = request("POST", "/auth/token", Some("tok"), Some(&[("code", "a b")]));
        assert_eq!(req.url, "https://api.wemod.com/auth/token");
        assert_eq!(req.body.as_deref(), Some("code=a+b"));
        let has = |k: &str, v: &str| req.headers.iter().any(|(hk, hv)| hk == k && hv == v);
        assert!(has("Accept", "application/json"));
        assert!(has("Authorization", "Bearer tok"));
        assert!(has("Content-Type", "application/x-www-form-urlencoded"));
    }
}
