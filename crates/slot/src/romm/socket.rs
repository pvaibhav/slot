use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore, SignatureScheme,
    StreamOwned,
};
use tungstenite::{Message, WebSocket};

use super::api::ApiError;
use super::conf::{Protocol, RommConf};
use crate::update::CONNECT_TIMEOUT;

const PATH: &str = "/ws/socket.io/?EIO=4&transport=websocket";
const NAMESPACE: &str = "/devices,";
const READ_TICK: Duration = Duration::from_millis(500);
const PING_INTERVAL_MS: u64 = 25_000;
const PING_TIMEOUT_MS: u64 = 60_000;

#[derive(Clone, Debug, PartialEq)]
pub enum Push {
    Queued,
    Cancelled(String),
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    Open { silence: Duration },
    Ping,
    Connected,
    Push(Push),
}

pub trait Source: Send {
    fn read(&mut self) -> Result<Option<Push>, ApiError>;
}

pub fn parse_frame(text: &str) -> Vec<Frame> {
    let frame = match text.as_bytes().first() {
        Some(b'0') => Some(open(&text[1..])),
        Some(b'1') => Some(Frame::Push(Push::Closed)),
        Some(b'2') => Some(Frame::Ping),
        Some(b'4') => packet(&text[1..]),
        _ => None,
    };
    frame.into_iter().collect()
}

fn open(json: &str) -> Frame {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let interval = v["pingInterval"].as_u64().unwrap_or(PING_INTERVAL_MS);
    let timeout = v["pingTimeout"].as_u64().unwrap_or(PING_TIMEOUT_MS);
    Frame::Open {
        silence: Duration::from_millis(interval + timeout),
    }
}

fn packet(text: &str) -> Option<Frame> {
    let (kind, rest) = text.split_at_checked(1)?;
    let body = rest.strip_prefix(NAMESPACE)?;
    match kind {
        "0" => Some(Frame::Connected),
        "1" | "4" => Some(Frame::Push(Push::Closed)),
        "2" => event(body).map(Frame::Push),
        _ => None,
    }
}

fn event(json: &str) -> Option<Push> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    match v[0].as_str()? {
        "install:queued" => Some(Push::Queued),
        "install:cancelled" => v[1]["id"]
            .as_str()
            .map(|id| Push::Cancelled(id.to_string())),
        _ => None,
    }
}

pub fn connect_frame(token: &str) -> String {
    format!("40{NAMESPACE}{}", serde_json::json!({ "token": token }))
}

pub fn socket_url(conf: &RommConf) -> String {
    let origin = conf.origin();
    let rest = origin.split_once("://").map_or("", |(_, rest)| rest);
    let scheme = match conf.protocol {
        Protocol::Https => "wss",
        Protocol::Http => "ws",
    };
    format!("{scheme}://{rest}{PATH}")
}

fn address(conf: &RommConf) -> (&str, u16) {
    let host = conf.host();
    if let Some(port) = conf.port {
        return (host, port);
    }
    if let Some((name, port)) = host
        .rsplit_once(':')
        .and_then(|(name, port)| Some((name, port.parse().ok()?)))
    {
        return (name, port);
    }
    match conf.protocol {
        Protocol::Https => (host, 443),
        Protocol::Http => (host, 80),
    }
}

enum Wire {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Wire {
    fn tcp(&self) -> &TcpStream {
        match self {
            Wire::Plain(tcp) => tcp,
            Wire::Tls(tls) => &tls.sock,
        }
    }

    fn timeout(&self, read: Duration) -> io::Result<()> {
        self.tcp().set_read_timeout(Some(read))?;
        self.tcp().set_write_timeout(Some(CONNECT_TIMEOUT))
    }
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Wire::Plain(tcp) => tcp.read(buf),
            Wire::Tls(tls) => tls.read(buf),
        }
    }
}

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Wire::Plain(tcp) => tcp.write(buf),
            Wire::Tls(tls) => tls.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Wire::Plain(tcp) => tcp.flush(),
            Wire::Tls(tls) => tls.flush(),
        }
    }
}

#[derive(Debug)]
struct NoVerify(Arc<CryptoProvider>);

impl ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn tls_config(self_signed: bool) -> Result<ClientConfig, ApiError> {
    let provider = Arc::new(ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|_| ApiError::Offline)?;
    Ok(match self_signed {
        true => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerify(provider)))
            .with_no_client_auth(),
        false => {
            let roots = RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            };
            builder.with_root_certificates(roots).with_no_client_auth()
        }
    })
}

fn tcp(host: &str, port: u16) -> Result<TcpStream, ApiError> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|_| ApiError::Offline)?;
    addrs
        .into_iter()
        .find_map(|addr| TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).ok())
        .ok_or(ApiError::Offline)
}

fn wire(conf: &RommConf) -> Result<Wire, ApiError> {
    let (host, port) = address(conf);
    let tcp = tcp(host, port)?;
    let _ = tcp.set_nodelay(true);
    let wire = match conf.protocol {
        Protocol::Http => Wire::Plain(tcp),
        Protocol::Https => {
            let config = tls_config(conf.self_signed)?;
            let name = ServerName::try_from(host.to_string()).map_err(|_| ApiError::Offline)?;
            let tls =
                ClientConnection::new(Arc::new(config), name).map_err(|_| ApiError::Offline)?;
            Wire::Tls(Box::new(StreamOwned::new(tls, tcp)))
        }
    };
    wire.timeout(CONNECT_TIMEOUT)
        .map_err(|_| ApiError::Offline)?;
    Ok(wire)
}

pub struct Socket {
    ws: WebSocket<Wire>,
    silence: Duration,
    heard: Instant,
}

pub fn connect(conf: &RommConf) -> Result<Socket, ApiError> {
    let token = conf.token.as_deref().ok_or(ApiError::Unauthorized)?;
    let wire = wire(conf)?;
    let (ws, _) =
        tungstenite::client::client(socket_url(conf), wire).map_err(|_| ApiError::Offline)?;
    let mut socket = Socket {
        ws,
        silence: Duration::from_millis(PING_INTERVAL_MS + PING_TIMEOUT_MS),
        heard: Instant::now(),
    };
    socket.join(token)?;
    socket
        .ws
        .get_ref()
        .timeout(READ_TICK)
        .map_err(|_| ApiError::Offline)?;
    Ok(socket)
}

pub fn dial(conf: &RommConf) -> Result<Box<dyn Source>, ApiError> {
    Ok(Box::new(connect(conf)?))
}

fn waiting(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

impl Socket {
    fn join(&mut self, token: &str) -> Result<(), ApiError> {
        loop {
            let text = match self.ws.read() {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) | Err(_) => return Err(ApiError::Offline),
                Ok(_) => continue,
            };
            for frame in parse_frame(&text) {
                match frame {
                    Frame::Open { silence } => {
                        self.silence = silence;
                        self.send(connect_frame(token))?;
                    }
                    Frame::Ping => self.send("3".to_string())?,
                    Frame::Connected => return Ok(()),
                    Frame::Push(Push::Closed) => return Err(ApiError::Offline),
                    Frame::Push(_) => {}
                }
            }
        }
    }

    fn send(&mut self, text: String) -> Result<(), ApiError> {
        match self.ws.send(Message::text(text)) {
            Ok(()) => Ok(()),
            Err(tungstenite::Error::Io(e)) if waiting(&e) => Ok(()),
            Err(_) => Err(ApiError::Offline),
        }
    }
}

impl Source for Socket {
    fn read(&mut self) -> Result<Option<Push>, ApiError> {
        let message = match self.ws.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(e)) if waiting(&e) => {
                return match self.heard.elapsed() < self.silence {
                    true => Ok(None),
                    false => Err(ApiError::Offline),
                };
            }
            Err(_) => return Err(ApiError::Offline),
        };
        self.heard = Instant::now();
        let text = match message {
            Message::Text(text) => text,
            Message::Close(_) => return Ok(Some(Push::Closed)),
            _ => return Ok(None),
        };
        for frame in parse_frame(&text) {
            match frame {
                Frame::Ping => self.send("3".to_string())?,
                Frame::Push(push) => return Ok(Some(push)),
                Frame::Open { .. } | Frame::Connected => {}
            }
        }
        Ok(None)
    }
}
