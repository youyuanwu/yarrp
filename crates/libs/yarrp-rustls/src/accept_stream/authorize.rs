pub trait ClientAuthorizor: Send + Sync + 'static {
    fn on_connect(&self, conn: &rustls::ServerConnection) -> Result<(), crate::Error>;
}

pub trait AuthorizeCallback:
    Fn(&rustls::ServerConnection) -> Result<(), crate::Error> + Send + Sync + 'static
{
}

impl<T> AuthorizeCallback for T where
    T: Fn(&rustls::ServerConnection) -> Result<(), crate::Error> + Send + Sync + 'static
{
}

pub struct LambdaClientAuthorizor<T: AuthorizeCallback> {
    f: T,
}

impl<T: AuthorizeCallback> LambdaClientAuthorizor<T> {
    pub fn new(f: T) -> Self {
        Self { f }
    }
}

impl<T: AuthorizeCallback> ClientAuthorizor for LambdaClientAuthorizor<T> {
    fn on_connect(&self, conn: &rustls::ServerConnection) -> Result<(), crate::Error> {
        (self.f)(conn)
    }
}
