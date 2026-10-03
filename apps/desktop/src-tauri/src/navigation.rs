use tauri::Url;

/// Top-level document navigations stay on the packaged app origin.
/// Debug builds may also stay on the configured dev-server origin.
/// Anything else, including the IPC host, is rejected.
pub fn app_navigation_allowed(url: &Url, dev_url: Option<&Url>) -> bool {
    if is_packaged_app_origin(url) {
        return true;
    }

    #[cfg(debug_assertions)]
    if let Some(dev) = dev_url {
        return same_origin(url, dev);
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = dev_url;
    }

    false
}

fn is_packaged_app_origin(url: &Url) -> bool {
    if url.port().is_some() {
        return false;
    }
    match (url.scheme(), url.host_str()) {
        ("http" | "https", Some("tauri.localhost")) => true,
        ("tauri", Some("localhost")) => true,
        _ => false,
    }
}

fn same_origin(url: &Url, allowed: &Url) -> bool {
    url.scheme() == allowed.scheme()
        && url.host() == allowed.host()
        && url.port_or_known_default() == allowed.port_or_known_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(raw: &str) -> Url {
        Url::parse(raw).expect(raw)
    }

    #[test]
    fn allows_packaged_tauri_localhost() {
        assert!(app_navigation_allowed(
            &url("http://tauri.localhost/index.html"),
            None
        ));
        assert!(app_navigation_allowed(
            &url("https://tauri.localhost/"),
            None
        ));
        assert!(app_navigation_allowed(&url("tauri://localhost/"), None));
    }

    #[test]
    fn rejects_remote_and_ipc_document_navigations() {
        assert!(!app_navigation_allowed(&url("https://evil.example/app"), None));
        assert!(!app_navigation_allowed(&url("http://ipc.localhost/"), None));
        assert!(!app_navigation_allowed(
            &url("https://ipc.localhost/invoke"),
            None
        ));
        assert!(!app_navigation_allowed(
            &url("http://tauri.localhost.evil.example/"),
            None
        ));
        assert!(!app_navigation_allowed(
            &url("http://tauri.localhost:5173/"),
            None
        ));
    }

    #[test]
    fn debug_allows_only_the_configured_dev_origin() {
        let dev = url("http://localhost:5173/");
        let page = url("http://localhost:5173/src/main.tsx");
        let other_port = url("http://localhost:5174/");
        let https_dev = url("https://localhost:5173/");

        if cfg!(debug_assertions) {
            assert!(app_navigation_allowed(&page, Some(&dev)));
            assert!(!app_navigation_allowed(&other_port, Some(&dev)));
            assert!(!app_navigation_allowed(&https_dev, Some(&dev)));
            assert!(!app_navigation_allowed(&page, None));
        } else {
            assert!(!app_navigation_allowed(&page, Some(&dev)));
        }
    }
}
