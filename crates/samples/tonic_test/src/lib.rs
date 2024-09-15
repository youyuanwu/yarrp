tonic::include_proto!("helloworld"); // The string specified here must match the proto package name

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use proxy::{connector::TcpConnector, CancellationToken};
    use tokio::net::TcpListener;
    use tonic::transport::Server;

    use crate::{HelloReply, HelloRequest};

    #[derive(Default)]
    pub struct HelloWorldService {}

    #[tonic::async_trait]
    impl super::greeter_server::Greeter for HelloWorldService {
        async fn say_hello(
            &self,
            req: tonic::Request<HelloRequest>,
        ) -> Result<tonic::Response<HelloReply>, tonic::Status> {
            let name = req.into_inner().name;
            Ok(tonic::Response::new(HelloReply {
                message: format!("hello {}", name),
            }))
        }
    }

    async fn run_hello_server(token: CancellationToken) -> Result<(), tonic::transport::Error> {
        let addr = "[::1]:50051".parse().unwrap();
        let greeter = HelloWorldService::default();

        println!("GreeterServer listening on {}", addr);

        Server::builder()
            .add_service(crate::greeter_server::GreeterServer::new(greeter))
            .serve_with_shutdown(addr, async move { token.cancelled().await })
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn basic_test() {
        let sv_token = CancellationToken::new();
        let sv_token_cp = sv_token.clone();
        let rt = tokio::runtime::Handle::current();
        // Run tonic server
        let sv_h = rt.spawn(async move {
            run_hello_server(sv_token_cp).await.unwrap();
        });

        // run proxy route to tonic
        let sv_token_cp2 = sv_token.clone();
        let proxy_h =
            rt.spawn(async {
                let addr: SocketAddr = "[::1]:50052".parse().unwrap();
                let incoming = TcpListener::bind(&addr).await.unwrap();
                let stream = tokio_stream::wrappers::TcpListenerStream::new(incoming);
                let conn = TcpConnector::new("[::1]:50051".parse().unwrap()); // tonic addr
                let service = proxy::proxy_service::ProxyService::new(conn).await;
                proxy::serve_with_incoming(stream, service, async move {
                    sv_token_cp2.cancelled().await
                })
                .await
                .unwrap();
            });

        tokio::time::sleep(Duration::from_secs(2)).await;

        // send request to proxy
        let mut client = crate::greeter_client::GreeterClient::connect("http://[::1]:50052")
            .await
            .unwrap();
        let request = tonic::Request::new(HelloRequest {
            name: "Tonic".into(),
        });
        let response = client.say_hello(request).await.unwrap();

        println!("RESPONSE={:?}", response);

        sv_token.cancel();
        sv_h.await.unwrap();
        proxy_h.await.unwrap();
    }
}
