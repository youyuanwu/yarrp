use hyper::body::Incoming;
use hyper_util::client::legacy::{connect::Connect, Client, ResponseFuture};

use tower::Service;

// Proxy hyper service for uds.
// Pipes all hyper requests into the uds client
#[derive(Debug, Clone)]
pub struct ProxyService<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    c: Client<C, Incoming>,
}

impl<C> ProxyService<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    pub fn new(conn: C) -> Self {
        Self {
            c: make_client(conn),
        }
    }
}

impl<C> hyper::service::Service<hyper::Request<Incoming>> for ProxyService<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    type Response = hyper::Response<Incoming>;

    type Error = hyper_util::client::legacy::Error;
    type Future = ResponseFuture;

    fn call(&self, req: hyper::Request<Incoming>) -> Self::Future {
        let mut c = self.c.clone();
        c.call(req)
    }
}

/// Makes the http client that poxy request to.
fn make_client<R: hyper::body::Body + Send, C: Connect + Clone>(conn: C) -> Client<C, R>
where
    <R as hyper::body::Body>::Data: Send,
{
    // TODO: write a hyper client directly and replace legacy client.
    hyper_util::client::legacy::Builder::new(hyper_util::rt::TokioExecutor::new())
        .pool_idle_timeout(std::time::Duration::from_secs(3))
        .pool_timer(hyper_util::rt::TokioTimer::new())
        .http2_only(true)
        .build::<_, R>(conn)
}
