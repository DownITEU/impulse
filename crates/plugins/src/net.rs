//! The app's GET for a plugin, a guest's or a download's: on the plugin's hosts only, its outcome as the wire's error
//! kinds.

use std::sync::Arc;

use sdk::wire::{self, ErrorKind};

use crate::{runtime::Net, scope::Scope};

pub fn net(scope: Arc<Scope>) -> Net {
    Arc::new(move |url, limit, left| {
        let error = |kind, message: String| Err(wire::Error::new(kind, message));
        match fetch::guarded(url, &|host| scope.hosts.allows(host), limit, left) {
            Ok(response) if (200..300).contains(&response.status) => Ok(response.body),
            Ok(response) => error(status_kind(response.status), format!("HTTP {}", response.status)),
            Err(e) if e.downcast_ref::<fetch::Refused>().is_some() => error(ErrorKind::Refused, format!("{e:#}")),
            Err(e) => error(ErrorKind::Offline, format!("{e:#}")),
        }
    })
}

/// Too many requests is worth asking again later: `Offline`, not `Refused`.
fn status_kind(status: u16) -> ErrorKind {
    match status {
        404 | 410 => ErrorKind::NotFound,
        401 | 403 => ErrorKind::Refused,
        429 | 500.. => ErrorKind::Offline,
        _ => ErrorKind::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_becomes_the_kind_that_says_whether_to_try_again() {
        let kinds = [(404, ErrorKind::NotFound), (410, ErrorKind::NotFound), (401, ErrorKind::Refused), (403, ErrorKind::Refused)];
        let again = [(429, ErrorKind::Offline), (500, ErrorKind::Offline), (503, ErrorKind::Offline), (418, ErrorKind::Internal)];
        for (status, kind) in kinds.into_iter().chain(again) {
            assert_eq!(status_kind(status), kind, "HTTP {status}");
        }
    }
}
