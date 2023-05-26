Tower middleware to sanitize paths.

Any sort of path [traversal techniques](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/)
used to access the underlying filesystem will be removed from the request's paths. For example, a request with `/../../passwd`
will become `/passwd` before being passed to inner services.

# Example

```
use http::{Request, Response, StatusCode};
use hyper::Body;
use std::{iter::once, convert::Infallible};
use tower::{ServiceBuilder, Service, ServiceExt};
use tower_sanitize_path::SanitizePathLayer;

# #[tokio::main]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
async fn handle(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    // `req.uri().path()` will not be usable to traverse the filesystem
    # Ok(Response::new(Body::empty()))
}

let mut service = ServiceBuilder::new()
    // sanitize the paths
    .layer(SanitizePathLayer)
    .service_fn(handle);

// call the service
let request = Request::builder()
    // `handle` will see `/secret`
    .uri("/../../secret")
    .body(Body::empty())?;

service.ready().await?.call(request).await?;
#
# Ok(())
# }
```
