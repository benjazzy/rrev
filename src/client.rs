use crate::connection::ConnectionHdl;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite;
use url::Url;

pub async fn connect<
    OurReq: Serialize + Send + 'static,
    OurRep: Serialize + Send + 'static,
    OurEvent: Serialize + Send + 'static,
    TheirReq: for<'a> Deserialize<'a>,
    TheirRep: for<'a> Deserialize<'a>,
    TheirEvent: for<'a> Deserialize<'a>,
>(
    url: Url,
) -> Result<
    ConnectionHdl<OurReq, OurRep, OurEvent, TheirReq, TheirRep, TheirEvent>,
    tungstenite::error::Error,
> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;

    // let hdl = ConnectionHdl::new(ws_stream);
    todo!()
}

#[cfg(test)]
mod tests {
    use crate::client::connect;
    use crate::connection;
    use std::ops::ControlFlow;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
    use url::Url;

    type ConnectionHdl = connection::ConnectionHdl<String, String, String, String, String, String>;

    #[tokio::test]
    async fn check_connect() {
        let event = "test event";

        let (server_tx, mut server_rx) = mpsc::channel(1);
        let (close_tx, close_rx) = mpsc::channel(1);

        tokio::spawn(server(server_tx, close_rx));

        let message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting address")
            .expect("Server sent empty message.");

        let addr = match message {
            ServerMessage::Address(a) => a,
            ServerMessage::NewConnection(_) => panic!("Server sent incorrect message type."),
        };
        let url = format!("ws://{addr}");

        let client_hdl = connect::<String, String, String, String, String, String>(
            Url::parse(url.as_str()).expect("Problem parsing url"),
        )
        .await
        .expect("Problem connecting to the server");

        let message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting connection handle")
            .expect("Server sent empty message.");

        let server_hdl = match message {
            ServerMessage::Address(_) => panic!("Server sent incorrect message type"),
            ServerMessage::NewConnection(c) => c,
        };

        let (event_tx, mut event_rx) = mpsc::channel(1);

        client_hdl.register_event_listener(event_tx.clone()).await;
        server_hdl.register_event_listener(event_tx.clone()).await;

        client_hdl.event(event.to_string());
        assert_eq!(event_rx.recv().await, Some(event.to_string()));

        server_hdl.event(event.to_string());
        assert_eq!(event_rx.recv().await, Some(event.to_string()));
    }

    #[derive(Debug, Clone)]
    enum ServerMessage {
        NewConnection(ConnectionHdl),
        Address(String),
    }

    async fn server(tx: mpsc::Sender<ServerMessage>, mut rx: mpsc::Receiver<()>) {
        let socket = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Error binding on socket address");
        let addr = socket
            .local_addr()
            .expect("Error getting socket listen address");

        println!("Server listening on {addr}");

        tx.send(ServerMessage::Address(addr.to_string()))
            .await
            .expect("Problem sending address to test.");

        loop {
            // let control = tokio::select! {
            //     Some(_) = rx.recv() => { ControlFlow::Break(()) }
            // };
            let control = tokio::select! {
                stream = socket.accept() => {
                    let (stream, _) = stream.expect("Problem accepting connection");
                    let maybe_tls = MaybeTlsStream::Plain(stream);
                    let ws_stream = tokio_tungstenite::accept_async(maybe_tls).await
                        .expect("Problem upgrading stream to a websocket.");

                    let connection_hdl = ConnectionHdl::new(ws_stream).await;
                    tx.send(ServerMessage::NewConnection(connection_hdl));
                    ControlFlow::Continue(())
                },
                _ = rx.recv() => { ControlFlow::Break(()) }
            };

            if let ControlFlow::Break(()) = control {
                break;
            }
        }

        println!("Server closing");
    }
}
