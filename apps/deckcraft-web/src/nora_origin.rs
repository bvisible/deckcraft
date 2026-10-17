// //// Neoffice — added file (no upstream equivalent): where the Nora pane of the browser build may send its calls.

//! Where the Nora pane's calls may go, and which of them may carry the page's credentials. The rules,
//! common to Neoffice's editors:
//! - `?nora` with no value (or `1`, `true`) is the page's own origin (`location.origin`);
//! - `?nora=<origin>` is taken only when it is the page's own origin, or when the page and that origin
//!   are both loopback (`127.0.0.1`, `localhost`, `[::1]`: the workshop's stand-in server); any other
//!   origin can only come from `window.deckcraftNora.origin`, which the hosting page sets;
//! - the page's credentials (a Frappe page's CSRF token, `window.deckcraftNora.headers`) go only with a
//!   call to the page's own origin.
//!
//! So a link cannot send the person's messages, what Nora reads of the presentation and the pictures of
//! its slides, with their session's token, to a server the link names. The rules have no browser type in
//! them, so they are tested natively; `web/nora.rs` applies them.

/// The origin Nora's calls go to when the page's address has `nora=<param>` (`param` decoded, empty for a
/// bare `?nora`), for a page whose own origin is `page`. `Err` says why an origin the address names is not
/// taken: the caller then uses the page's own origin.
pub fn from_param(param: &str, page: &str) -> Result<String, String> {
    // A page without an http(s) origin of its own (`location.origin` is "null") calls a relative address.
    let own = || if parse(page).is_some() { page.to_string() } else { String::new() };
    if matches!(param, "" | "1" | "true") {
        return Ok(own());
    }
    let named = param.trim().trim_end_matches('/');
    match (parse(named), parse(page)) {
        (Some(origin), Some(page_origin)) if origin == page_origin => Ok(own()),
        (Some(origin), Some(page_origin)) if is_loopback(&origin.host) && is_loopback(&page_origin.host) => Ok(named.to_string()),
        _ => Err(format!(
            "the page's address names {named:?} for Nora: only the page's own origin, or a loopback one from a loopback page, is taken from the \
             address; the hosting page names another one in `window.deckcraftNora.origin`"
        )),
    }
}

/// Whether a call to `target` may carry the page's own credentials: only a call to the page's own origin.
pub fn is_page_origin(target: &str, page: &str) -> bool {
    match (parse(target.trim().trim_end_matches('/')), parse(page)) {
        (Some(target), Some(page)) => target == page,
        _ => false,
    }
}

/// An http(s) origin: scheme, host and port, compared as the browser compares origins.
#[derive(Debug, PartialEq, Eq)]
struct Origin {
    scheme: String,
    host: String,
    port: u16,
}

/// `scheme://host[:port]` and nothing else (no user, path, query or fragment); `None` otherwise.
fn parse(text: &str) -> Option<Origin> {
    let (scheme, rest) = text.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let default_port = match scheme.as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    if rest.is_empty() || rest.contains(['/', '?', '#', '@', '\\']) {
        return None;
    }
    let (host, port) = match rest.strip_prefix('[') {
        // An IPv6 address, in brackets.
        Some(after) => {
            let (inner, tail) = after.split_once(']')?;
            let port = if tail.is_empty() { default_port } else { tail.strip_prefix(':')?.parse().ok()? };
            (format!("[{}]", inner.to_ascii_lowercase()), port)
        }
        None => match rest.rsplit_once(':') {
            Some((host, port)) => (host.to_ascii_lowercase(), port.parse().ok()?),
            None => (rest.to_ascii_lowercase(), default_port),
        },
    };
    if host.is_empty() || host == "[]" || (!host.starts_with('[') && host.contains(':')) {
        return None;
    }
    Some(Origin { scheme, host, port })
}

/// The loopback hosts the workshop's stand-in server runs on.
fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const PAGE: &str = "https://drive.example";
    const LOOPBACK_PAGE: &str = "http://127.0.0.1:8791";

    #[test]
    fn a_bare_nora_is_the_page_s_own_origin() {
        for param in ["", "1", "true"] {
            assert_eq!(from_param(param, PAGE), Ok(PAGE.to_string()), "{param:?}");
        }
    }

    #[test]
    fn the_page_s_own_origin_may_be_named() {
        for param in ["https://drive.example", "https://drive.example/", "HTTPS://Drive.Example", "https://drive.example:443"] {
            assert!(from_param(param, PAGE).is_ok(), "{param:?}");
        }
    }

    #[test]
    fn any_other_origin_named_by_the_address_is_refused() {
        for param in [
            "https://evil.example",
            "http://drive.example",
            "https://drive.example:8443",
            "https://drive.example.evil.example",
            "https://drive.example@evil.example",
            "https://evil.example/https://drive.example",
            "//evil.example",
            "javascript:alert(1)",
            "evil.example",
            "http://127.0.0.1:8773",
        ] {
            let refused = from_param(param, PAGE);
            assert!(refused.is_err(), "{param:?} was taken: {refused:?}");
        }
    }

    #[test]
    fn a_loopback_page_may_name_a_loopback_origin_for_the_workshop_s_mock() {
        for param in ["http://127.0.0.1:8773", "http://localhost:8773", "http://[::1]:8773", "http://LOCALHOST:8773/"] {
            assert!(from_param(param, LOOPBACK_PAGE).is_ok(), "{param:?}");
        }
        assert!(from_param("http://[::1]:8773", "http://localhost:8080").is_ok());
        for param in ["https://evil.example", "http://127.0.0.1.evil.example:8773", "http://127.0.0.2:8773", "http://localhost@evil.example"] {
            assert!(from_param(param, LOOPBACK_PAGE).is_err(), "{param:?}");
        }
    }

    #[test]
    fn the_page_s_credentials_go_only_to_its_own_origin() {
        assert!(is_page_origin("https://drive.example", PAGE));
        assert!(is_page_origin("https://drive.example/", PAGE));
        assert!(!is_page_origin("https://evil.example", PAGE));
        assert!(!is_page_origin("http://drive.example", PAGE), "another scheme is another origin");
        assert!(!is_page_origin("http://127.0.0.1:8773", LOOPBACK_PAGE), "another port is another origin, loopback or not");
        assert!(!is_page_origin("not an origin", PAGE));
    }
}
