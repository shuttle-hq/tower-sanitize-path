use std::{
    borrow::Cow,
    path::{Component, PathBuf},
    str::FromStr,
};

use http::Uri;
use url_escape::decode;

fn sanitize_path(uri: &mut Uri) {
    let path = uri.path();
    let path_decoded = decode(path);
    let path_buf = PathBuf::from_str(&path_decoded).expect("infallible");

    let new_path = path_buf
        .components()
        .filter(|c| matches!(c, Component::RootDir | Component::Normal(_)))
        .collect::<PathBuf>()
        .display()
        .to_string();

    if path == new_path {
        return;
    }

    let mut parts = uri.clone().into_parts();

    let new_path_and_query = if let Some(path_and_query) = parts.path_and_query {
        let new_path_and_query = if let Some(query) = path_and_query.query() {
            Cow::Owned(format!("{new_path}?{query}"))
        } else {
            new_path.into()
        }
        .parse()
        .expect("url to still be valid");

        Some(new_path_and_query)
    } else {
        None
    };

    parts.path_and_query = new_path_and_query;
    if let Ok(new_uri) = Uri::from_parts(parts) {
        *uri = new_uri;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_path() {
        let mut uri = "/".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/");
    }

    #[test]
    fn maintain_query() {
        let mut uri = "/?test".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/?test");
    }

    #[test]
    fn path_maintain_query() {
        let mut uri = "/path?test=true".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path?test=true");
    }

    #[test]
    fn remove_path_parent_transversal() {
        let mut uri = "/../../path".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path");
    }

    #[test]
    fn remove_path_parent_transversal_maintain_query() {
        let mut uri = "/../../path?name=John".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path?name=John");
    }

    #[test]
    fn remove_path_current_transversal() {
        let mut uri = "/.././path".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path");
    }

    #[test]
    fn remove_path_encoded_transversal() {
        let mut uri = "/..%2f..%2fpath".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path");
    }
}
