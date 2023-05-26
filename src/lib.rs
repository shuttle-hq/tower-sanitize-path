#![doc = include_str!("../README.md")]

use std::{
    borrow::Cow,
    path::{Component, PathBuf},
    str::FromStr,
    task::{Context, Poll},
};

use http::{Request, Response, Uri};
use tower_layer::Layer;
use tower_service::Service;
use url_escape::decode;

/// Layer that applies [`SanitizePath`] which sanitizes paths.
///
/// See the [module docs](self) for more details.
pub struct SanitizePathLayer;

impl<S> Layer<S> for SanitizePathLayer {
    type Service = SanitizePath<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SanitizePath::sanitize_paths(inner)
    }
}

/// Middleware to remove filesystem path traversals attempts from URL paths.
///
/// See the [module docs](self) for more details.
#[derive(Clone, Copy, Debug)]
pub struct SanitizePath<S> {
    inner: S,
}

impl<S> SanitizePath<S> {
    /// Sanitize all paths for the given service.
    ///
    /// This will make all paths on the URL safe for the service to consume.
    pub fn sanitize_paths(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for SanitizePath<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        sanitize_path(req.uri_mut());

        self.inner.call(req)
    }
}

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
    use std::convert::Infallible;

    use tower::{ServiceBuilder, ServiceExt};

    use super::*;

    #[tokio::test]
    async fn layer() {
        async fn handle(request: Request<()>) -> Result<Response<String>, Infallible> {
            Ok(Response::new(request.uri().to_string()))
        }

        let mut svc = ServiceBuilder::new()
            .layer(SanitizePathLayer)
            .service_fn(handle);

        let body = svc
            .ready()
            .await
            .unwrap()
            .call(Request::builder().uri("/../../secret").body(()).unwrap())
            .await
            .unwrap()
            .into_body();

        assert_eq!(body, "/secret");
    }

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
    fn remove_path_parent_traversal() {
        let mut uri = "/../../path".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path");
    }

    #[test]
    fn remove_path_parent_traversal_maintain_query() {
        let mut uri = "/../../path?name=John".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path?name=John");
    }

    #[test]
    fn remove_path_current_traversal() {
        let mut uri = "/.././path".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path");
    }

    #[test]
    fn remove_path_encoded_traversal() {
        let mut uri = "/..%2f..%2fpath".parse().unwrap();
        sanitize_path(&mut uri);

        assert_eq!(uri, "/path");
    }
}
