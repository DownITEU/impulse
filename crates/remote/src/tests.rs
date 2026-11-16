use std::{
    cell::RefCell,
    net::{SocketAddr, TcpListener as StdListener},
};

use async_io::block_on;
use async_net::TcpStream;
use async_tungstenite::{
    client_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use client::api::{Ask, Command, RunState, View, Views, link};
use futures::{
    AsyncReadExt, AsyncWriteExt, Stream, StreamExt,
    future::{Either, select},
    pin_mut,
};

use super::Server;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Pairs the code `c0de`, knows `TOKEN` as device 1, keeps the commands and answers each with a library.
async fn app(mut asks: impl Stream<Item = Ask> + Unpin, views: Views, commands: &RefCell<Vec<Command>>) {
    while let Some(ask) = asks.next().await {
        match ask {
            Ask::Pair { code, reply, .. } => drop(reply.send((code == "c0de").then(|| TOKEN.to_owned()))),
            Ask::Device { token, reply } => drop(reply.send((token == TOKEN).then_some(1))),
            Ask::Command { command, .. } => {
                commands.borrow_mut().push(command);
                views.publish(&View::Library { games: Vec::new() });
            }
            Ask::Art { reply, .. } => drop(reply.send(None)),
        }
    }
}

/// Runs `test` against a server on loopback and an `app`.
fn with_server(test: impl AsyncFnOnce(SocketAddr, &Views, &RefCell<Vec<Command>>)) {
    let server = Server::over(StdListener::bind("127.0.0.1:0").unwrap()).unwrap();
    let address = server.address();
    let (server_side, views, asks) = link();
    let commands = RefCell::new(Vec::new());
    block_on(async {
        let (serve, app) = (server.serve(server_side), app(asks, views.clone(), &commands));
        let test = test(address, &views, &commands);
        pin_mut!(serve, app, test);
        if let Either::Right(_) = select(test, select(serve, app)).await {
            panic!("the server or the app stopped first");
        }
    });
}

async fn http(address: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    answer
}

fn pair(address: SocketAddr, origin: &str, code: &str) -> String {
    format!("POST /pair HTTP/1.1\r\nHost: {address}\r\nOrigin: {origin}\r\nContent-Length: {}\r\n\r\n{code}", code.len())
}

#[test]
fn pairing_needs_the_host_the_origin_and_the_code() {
    with_server(async |address, _, _| {
        let origin = format!("http://{address}");
        assert!(http(address, "GET / HTTP/1.1\r\nHost: evil.example\r\n\r\n").await.starts_with("HTTP/1.1 421"));
        assert!(http(address, &format!("GET / HTTP/1.1\r\nHost: {address}\r\nHost: {address}\r\n\r\n")).await.is_empty());
        assert!(http(address, &pair(address, "http://evil.example", "c0de")).await.starts_with("HTTP/1.1 403"));
        assert!(http(address, &pair(address, &origin, "bad")).await.starts_with("HTTP/1.1 403"));
        let paired = http(address, &pair(address, &origin, "c0de")).await;
        assert!(paired.starts_with("HTTP/1.1 200"), "{paired}");
        assert!(paired.contains(&format!("Set-Cookie: impulse={TOKEN}; HttpOnly; SameSite=Strict")));
    });
}

#[test]
fn a_paired_socket_gets_the_views_sends_commands_and_ends_on_revoke() {
    with_server(async |address, views, commands| {
        views.publish(&View::Run { state: RunState::Live, game: Some(3), since: None });
        let socket = async |cookie: &str| {
            let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
            request.headers_mut().insert("Origin", format!("http://{address}").parse().unwrap());
            request.headers_mut().insert("Cookie", format!("impulse={cookie}").parse().unwrap());
            client_async(request, TcpStream::connect(address).await.unwrap()).await.map(|(socket, _)| socket)
        };
        assert!(socket(&"f".repeat(64)).await.is_err());
        let mut socket = socket(TOKEN).await.unwrap();
        let first = socket.next().await.unwrap().unwrap();
        assert!(first.to_text().unwrap().contains(r#""type":"run""#));
        socket.send(Message::text(r#"{ "type": "stop", "game": 3 }"#)).await.unwrap();
        assert!(socket.next().await.unwrap().unwrap().to_text().unwrap().contains("library"));
        assert_eq!(commands.borrow().as_slice(), [Command::Stop { game: 3 }]);
        views.close(1);
        assert!(matches!(socket.next().await, Some(Ok(Message::Close(_))) | None));
    });
}
