use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustls::crypto::ring;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use slot::romm::socket::{
    connect, connect_frame, parse_frame, socket_url, Frame, Push, Socket, Source,
};
use slot::romm::{http_api, ApiError, Protocol, RommConf};
use tungstenite::{accept, Message, WebSocket};

const CERT: &str = "-----BEGIN CERTIFICATE-----
MIIBdDCCARqgAwIBAgIUZLabxjjTp7VLA2JmlzE2XppPqIAwCgYIKoZIzj0EAwIw
FDESMBAGA1UEAwwJbG9jYWxob3N0MCAXDTI2MTEwMjA0MjA1OFoYDzIxMjYxMDA5
MDQyMDU4WjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwWTATBgcqhkjOPQIBBggqhkjO
PQMBBwNCAARL4stfeLp1sxApmzFJR4NbfDlPaiXWLkc7Bb29vHwlIt6TJOinFTyh
II2TqFQpjA+TRnoGjMboibPUcyH6RC1Lo0gwRjAdBgNVHQ4EFgQUy8OTM2+DQDBT
EIir836MB2UTn08wDwYDVR0TAQH/BAUwAwEB/zAUBgNVHREEDTALgglsb2NhbGhv
c3QwCgYIKoZIzj0EAwIDSAAwRQIgXYp9f94QS3bjy8oZsj1izZcOSYE4hSsWHa0Y
6S/R1jUCIQCXtG2r+KPFrDHebceFW1RinoZq/3d9j3KiVUZIwo/rDw==
-----END CERTIFICATE-----
";

const KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgiu5EPsg1Uoo7tAr4
iuv7gdtfcbYT1lSqRkrLjU4+Z1+hRANCAARL4stfeLp1sxApmzFJR4NbfDlPaiXW
Lkc7Bb29vHwlIt6TJOinFTyhII2TqFQpjA+TRnoGjMboibPUcyH6RC1L
-----END PRIVATE KEY-----
";

fn conf(protocol: Protocol, port: u16) -> RommConf {
    RommConf {
        protocol,
        server: "localhost".into(),
        port: Some(port),
        device_id: Some("d1".into()),
        token: Some("rmm_t".into()),
        ..RommConf::default()
    }
}

#[test]
fn engine_open_gives_the_silence_allowed() {
    assert_eq!(
        parse_frame(r#"0{"sid":"a","upgrades":[],"pingInterval":25000,"pingTimeout":60000}"#),
        vec![Frame::Open {
            silence: Duration::from_secs(85)
        }]
    );
}

#[test]
fn engine_ping_is_a_ping() {
    assert_eq!(parse_frame("2"), vec![Frame::Ping]);
}

#[test]
fn connect_carries_the_token_on_the_namespace() {
    assert_eq!(connect_frame("rmm_t"), r#"40/devices,{"token":"rmm_t"}"#);
}

#[test]
fn namespace_connect_is_connected() {
    assert_eq!(
        parse_frame(r#"40/devices,{"sid":"b"}"#),
        vec![Frame::Connected]
    );
}

#[test]
fn queued_event_is_queued() {
    assert_eq!(
        parse_frame(r#"42/devices,["install:queued",{"id":"x","rom_id":1}]"#),
        vec![Frame::Push(Push::Queued)]
    );
}

#[test]
fn cancelled_event_names_its_id() {
    assert_eq!(
        parse_frame(r#"42/devices,["install:cancelled",{"id":"x","rom_id":1}]"#),
        vec![Frame::Push(Push::Cancelled("x".into()))]
    );
}

#[test]
fn refusal_disconnect_and_engine_close_all_close() {
    for text in [
        r#"44/devices,{"message":"unauthorized"}"#,
        "41/devices,",
        "1",
    ] {
        assert_eq!(parse_frame(text), vec![Frame::Push(Push::Closed)], "{text}");
    }
}

#[test]
fn other_frames_and_namespaces_are_ignored() {
    for text in [
        "",
        "3",
        "6",
        "40",
        r#"42["install:queued",{"id":"x"}]"#,
        r#"42/other,["install:queued",{"id":"x"}]"#,
        r#"42/devices,["install:updated",{"id":"x"}]"#,
        "42/devices,not json",
    ] {
        assert!(parse_frame(text).is_empty(), "{text}");
    }
}

#[test]
fn url_follows_the_protocol() {
    assert_eq!(
        socket_url(&conf(Protocol::Https, 8443)),
        "wss://localhost:8443/ws/socket.io/?EIO=4&transport=websocket"
    );
    let mut plain = conf(Protocol::Http, 80);
    plain.port = None;
    assert_eq!(
        socket_url(&plain),
        "ws://localhost/ws/socket.io/?EIO=4&transport=websocket"
    );
}

fn text<S: std::io::Read + std::io::Write>(ws: &mut WebSocket<S>) -> String {
    loop {
        if let Message::Text(text) = ws.read().unwrap() {
            return text.to_string();
        }
    }
}

fn say<S: std::io::Read + std::io::Write>(ws: &mut WebSocket<S>, text: &str) {
    ws.send(Message::text(text.to_string())).unwrap();
}

fn open<S: std::io::Read + std::io::Write>(ws: &mut WebSocket<S>) -> String {
    say(
        ws,
        r#"0{"sid":"a","upgrades":[],"pingInterval":25000,"pingTimeout":60000}"#,
    );
    text(ws)
}

fn session<S: std::io::Read + std::io::Write>(ws: &mut WebSocket<S>) {
    assert_eq!(open(ws), r#"40/devices,{"token":"rmm_t"}"#);
    say(ws, r#"40/devices,{"sid":"b"}"#);
    say(ws, "2");
    assert_eq!(text(ws), "3");
    say(ws, r#"42/devices,["install:queued",{"id":"x","rom_id":1}]"#);
    say(
        ws,
        r#"42/devices,["install:cancelled",{"id":"x","rom_id":1}]"#,
    );
    say(ws, r#"44/devices,{"message":"gone"}"#);
}

fn next_push(socket: &mut Socket) -> Push {
    let began = Instant::now();
    loop {
        assert!(began.elapsed() < Duration::from_secs(10));
        if let Some(push) = socket.read().unwrap() {
            return push;
        }
    }
}

fn assert_session(socket: &mut Socket) {
    assert_eq!(next_push(socket), Push::Queued);
    assert_eq!(next_push(socket), Push::Cancelled("x".into()));
    assert_eq!(next_push(socket), Push::Closed);
}

fn plain_server(script: fn(&mut WebSocket<TcpStream>)) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut ws = accept(tcp).unwrap();
        script(&mut ws);
    });
    (port, server)
}

#[test]
fn plain_handshake_answers_pings_and_reads_events() {
    let (port, server) = plain_server(session);
    let mut socket = connect(&conf(Protocol::Http, port)).unwrap();
    assert_session(&mut socket);
    server.join().unwrap();
}

#[test]
fn refused_namespace_fails_the_connect() {
    let (port, server) = plain_server(|ws| {
        open(ws);
        say(ws, r#"44/devices,{"message":"unauthorized"}"#);
    });
    assert!(matches!(
        connect(&conf(Protocol::Http, port)),
        Err(ApiError::Offline)
    ));
    server.join().unwrap();
}

#[test]
fn a_quiet_read_returns_within_a_second() {
    let (port, server) = plain_server(|ws| {
        open(ws);
        say(ws, r#"40/devices,{"sid":"b"}"#);
        let _ = ws.read();
    });
    let mut socket = connect(&conf(Protocol::Http, port)).unwrap();
    let began = Instant::now();
    assert_eq!(socket.read(), Ok(None));
    assert!(began.elapsed() < Duration::from_secs(1));
    drop(socket);
    server.join().unwrap();
}

fn server_config() -> ServerConfig {
    ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from_pem_slice(CERT.as_bytes()).unwrap()],
            PrivateKeyDer::from_pem_slice(KEY.as_bytes()).unwrap(),
        )
        .unwrap()
}

fn tls_server(
    script: fn(&mut WebSocket<StreamOwned<ServerConnection, TcpStream>>),
) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = server_config();
    let server = std::thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let tls = ServerConnection::new(Arc::new(config)).unwrap();
        let Ok(mut ws) = accept(StreamOwned::new(tls, tcp)) else {
            return;
        };
        script(&mut ws);
    });
    (port, server)
}

#[test]
fn self_signed_connects_over_tls_when_accepted() {
    let (port, server) = tls_server(session);
    let mut accepted = conf(Protocol::Https, port);
    accepted.self_signed = true;
    let mut socket = connect(&accepted).unwrap();
    assert_session(&mut socket);
    server.join().unwrap();
}

#[test]
fn a_quiet_tls_read_times_out_and_still_reads_after() {
    let (port, server) = tls_server(|ws| {
        open(ws);
        say(ws, r#"40/devices,{"sid":"b"}"#);
        std::thread::sleep(Duration::from_millis(700));
        say(ws, r#"42/devices,["install:queued",{"id":"x","rom_id":1}]"#);
        let _ = ws.read();
    });
    let mut accepted = conf(Protocol::Https, port);
    accepted.self_signed = true;
    let mut socket = connect(&accepted).unwrap();
    assert_eq!(socket.read(), Ok(None));
    assert_eq!(next_push(&mut socket), Push::Queued);
    drop(socket);
    server.join().unwrap();
}

#[test]
fn a_redirect_to_https_on_the_same_host_keeps_the_bearer() {
    let tls = TcpListener::bind("127.0.0.1:0").unwrap();
    let tls_port = tls.local_addr().unwrap().port();
    let config = server_config();
    let server = std::thread::spawn(move || {
        let (tcp, _) = tls.accept().unwrap();
        let conn = ServerConnection::new(Arc::new(config)).unwrap();
        let mut stream = StreamOwned::new(conn, tcp);
        let head = head_of(&mut stream);
        let body = r#"{"SYSTEM":{"VERSION":"5.4.0"}}"#;
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
        head
    });
    let plain = TcpListener::bind("127.0.0.1:0").unwrap();
    let plain_port = plain.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut tcp, _) = plain.accept().unwrap();
        head_of(&mut tcp);
        let reply = format!(
            "HTTP/1.1 308 Permanent Redirect\r\nLocation: https://127.0.0.1:{tls_port}/api/heartbeat\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        tcp.write_all(reply.as_bytes()).unwrap();
    });
    let mut plain_conf = conf(Protocol::Http, plain_port);
    plain_conf.server = "127.0.0.1".into();
    plain_conf.self_signed = true;
    assert_eq!(http_api(&plain_conf).version(), Ok("5.4.0".to_string()));
    assert!(server
        .join()
        .unwrap()
        .to_ascii_lowercase()
        .contains("authorization: bearer rmm_t"));
}

fn head_of<S: Read>(stream: &mut S) -> String {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    while !seen.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap() == 0 {
            break;
        }
        seen.push(byte[0]);
    }
    String::from_utf8_lossy(&seen).to_string()
}

#[test]
fn self_signed_is_refused_unless_accepted() {
    let (port, server) = tls_server(session);
    assert!(matches!(
        connect(&conf(Protocol::Https, port)),
        Err(ApiError::Offline)
    ));
    server.join().unwrap();
}
