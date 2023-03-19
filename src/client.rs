use crate::connection::ConnectionHdl;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite;
use url::Url;

pub async fn connect<
    OurReq: Serialize + Send + 'static,
    OurRep: Serialize + Send + 'static,
    OurEvent: Serialize + Send + 'static,
    TheirReq: for<'a> Deserialize<'a> + Send + Clone + 'static,
    TheirRep: for<'a> Deserialize<'a> + Send + Clone + 'static,
    TheirEvent: for<'a> Deserialize<'a> + Send + Clone + 'static,
>(
    url: Url,
) -> Result<
    ConnectionHdl<OurReq, OurRep, OurEvent, TheirReq, TheirRep, TheirEvent>,
    tungstenite::error::Error,
> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
    let connection_hdl = ConnectionHdl::new(ws_stream).await;

    Ok(connection_hdl)
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

        client_hdl.event(event.to_string()).await;
        let recv_event = tokio::time::timeout(
            Duration::from_millis(100),
            event_rx.recv()
        ).await.expect("Timeout getting event.");
        println!("Event: {:#?}", recv_event);
        assert_eq!(recv_event, Some(event.to_string()));

        server_hdl.event(event.to_string()).await;
        let recv_event = tokio::time::timeout(
            Duration::from_millis(100),
            event_rx.recv()
        ).await.expect("Timeout getting event.");
        println!("Event: {:#?}", recv_event);
        assert_eq!(recv_event, Some(event.to_string()));
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
            let control = tokio::select! {
                stream = socket.accept() => {
                    let (stream, addr) = stream.expect("Problem accepting connection");
                    println!("Server accepting connection from {addr}");
                    let maybe_tls = MaybeTlsStream::Plain(stream);
                    let ws_stream = tokio_tungstenite::accept_async(maybe_tls).await
                        .expect("Problem upgrading stream to a websocket.");

                    let connection_hdl = ConnectionHdl::new(ws_stream).await;
                    tx.send(ServerMessage::NewConnection(connection_hdl)).await;
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
