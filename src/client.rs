pub use crate::connection::{ConnectionEvent, ConnectionHdl};
use crate::parser::Parser;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite;
use url::Url;

pub async fn connect<P: Parser>(
    url: Url,
    event_tx: mpsc::Sender<ConnectionEvent<P>>,
) -> Result<ConnectionHdl<P>, tungstenite::error::Error> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
    let connection_hdl = ConnectionHdl::new(ws_stream, event_tx).await;

    Ok(connection_hdl)
}

#[cfg(test)]
mod tests {
    use crate::client::connect;
    use crate::connection;
    use crate::connection::ConnectionEvent;
    use crate::parser::StringParser;
    use std::ops::ControlFlow;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_tungstenite::MaybeTlsStream;
    use url::Url;

    type ConnectionHdl = connection::ConnectionHdl<StringParser>;

    #[tokio::test]
    async fn check_connect() {
        let event = "test event";
        let (event_tx, mut event_rx) = mpsc::channel(1);

        let (server_tx, mut server_rx) = mpsc::channel(1);
        let (close_tx, close_rx) = mpsc::channel(1);

        tokio::spawn(server(server_tx, event_tx.clone(), close_rx));

        let message = tokio::time::timeout(Duration::from_millis(100), server_rx.recv())
            .await
            .expect("Timeout getting address")
            .expect("Server sent empty message.");

        let addr = match message {
            ServerMessage::Address(a) => a,
            ServerMessage::NewConnection(..) => panic!("Server sent incorrect message type."),
        };
        let url = format!("ws://{addr}");

        let client_hdl = connect::<StringParser>(
            Url::parse(url.as_str()).expect("Problem parsing url"),
            event_tx,
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

        //
        // client_hdl.register_event_listener(event_tx.clone()).await;
        // server_hdl.register_event_listener(event_tx.clone()).await;

        client_hdl.event(event.to_string()).await;
        let recv_event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .expect("Timeout getting event.")
            .expect("Problem unwrapping event.");
        println!("Event: {:#?}", recv_event);
        match recv_event {
            ConnectionEvent::Close => panic!("Got close from server."),
            ConnectionEvent::EventMessage(e) => assert_eq!(e, event.to_string()),
            ConnectionEvent::RequestMessage(_) => panic!("Got request from server."),
        }

        server_hdl.event(event.to_string()).await;
        let recv_event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .expect("Timeout getting event.")
            .expect("Problem unwrapping event.");
        println!("Event: {:#?}", recv_event);
        match recv_event {
            ConnectionEvent::Close => panic!("Got close from client."),
            ConnectionEvent::EventMessage(e) => assert_eq!(e, event.to_string()),
            ConnectionEvent::RequestMessage(_) => panic!("Got request from client."),
        }

        close_tx
            .send(())
            .await
            .expect("Problem sending close message to the server.");
    }

    #[derive(Debug, Clone)]
    enum ServerMessage {
        NewConnection(ConnectionHdl),
        Address(String),
    }

    async fn server(
        server_tx: mpsc::Sender<ServerMessage>,
        event_tx: mpsc::Sender<ConnectionEvent<StringParser>>,
        mut rx: mpsc::Receiver<()>,
    ) {
        let socket = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Error binding on socket address");
        let addr = socket
            .local_addr()
            .expect("Error getting socket listen address");

        println!("Server listening on {addr}");

        server_tx
            .send(ServerMessage::Address(addr.to_string()))
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

                    let connection_hdl = ConnectionHdl::new(ws_stream, event_tx.clone()).await;
                    server_tx.send(ServerMessage::NewConnection(connection_hdl)).await;
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
