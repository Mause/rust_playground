use crate::proxy::mock::Response;
use lazy_static::lazy_static;
use log::{error, info};
use native_tls::TlsStream;
use std::fmt::Write;
use std::io::{Read, Write as IOWrite};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;

mod identity;
mod mock;
pub use crate::proxy::mock::Mock;

lazy_static! {
    pub static ref STATE: Mutex<State> = Mutex::new(State::new());
}
const SERVER_ADDRESS_INTERNAL: &str = "127.0.0.1:1234";

pub struct Proxy {
    mocks: Vec<Mock>,
    listening_addr: Option<SocketAddr>,
    started: bool,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            mocks: Vec::new(),
            listening_addr: None,
            started: false,
        }
    }
}

impl Proxy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, mock: Mock) {
        if self.started {
            panic!("Cannot add mocks to a started proxy");
        }
        self.mocks.push(mock);
    }

    pub fn start(&mut self) {
        start_proxy(self);
    }

    /// Address and port of the local server.
    /// Can be used with `std::net::TcpStream`.
    ///
    /// The server will be started if necessary.
    pub fn address(&self) -> SocketAddr {
        let state = STATE.lock().map(|state| state.listening_addr);
        state
            .expect("state lock")
            .expect("server should be listening")
    }

    /// A local `http://…` URL of the server.
    ///
    /// The server will be started if necessary.
    pub fn url(&self) -> String {
        format!("http://{}", self.address())
    }

    pub fn get_certificate(&self) -> Vec<u8> {
        STATE.lock().unwrap().cert.to_pem().unwrap().clone()
    }
}

pub struct State {
    listening_addr: Option<SocketAddr>,
    identity: native_tls::Identity,
    cert: openssl::x509::X509,
}
impl State {
    fn new() -> Self {
        let (cert, identity) = create_identity();
        Self {
            listening_addr: None,
            identity: identity,
            cert: cert,
        }
    }
}

#[derive(Debug)]
struct Request {
    error: Option<String>,
    host: Option<String>,
    path: Option<String>,
    method: Option<String>,
    version: (u8, u8),
}

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("Request {")?;
        match &self.host {
            Some(e) => {
                e.fmt(f)?;
            }
            None => (),
        };
        f.write_char('}')?;
        Ok(())
    }
}
impl Request {
    fn is_ok(&self) -> bool {
        self.error().is_none()
    }
    fn error(&self) -> Option<&String> {
        self.error.as_ref()
    }

    fn from(mut stream: Box<&mut dyn Read>) -> Self {
        let mut request = Self {
            error: None,
            path: None,
            method: None,
            host: None,
            version: (0, 0),
        };

        let mut all_buf = Vec::new();

        loop {
            let mut buf = [0; 1024];

            let rlen = match stream.read(&mut buf) {
                Err(e) => Err(e.to_string()),
                Ok(0) => Err("Nothing to read.".into()),
                Ok(i) => Ok(i),
            }
            .map_err(|e| request.error = Some(e))
            .unwrap_or(0);
            if request.error().is_some() {
                break;
            }

            all_buf.extend_from_slice(&buf[..rlen]);

            if rlen < 1024 {
                break;
            }
        }

        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = httparse::Request::new(&mut headers);

        let _ = req
            .parse(&all_buf)
            .map_err(|err| {
                request.error = Some(err.to_string());
            })
            .map(|result| match result {
                httparse::Status::Complete(head_length) => {
                    request.method = req.method.map(|s| s.to_string());
                    request.path = req.path.map(|s| s.to_string());
                    if let Some(a @ 0..=1) = req.version {
                        request.version = (1, a);
                    }
                }
                httparse::Status::Partial => panic!("Incomplete request"),
            });

        request
    }
}

fn create_identity() -> (openssl::x509::X509, native_tls::Identity) {
    let cn = "discord.com";

    let (cert, key) = crate::proxy::identity::generateX509(cn, 5).unwrap();

    let password = "password";
    let encrypted = openssl::pkcs12::Pkcs12::builder()
        .build(password, cn, &key, &cert)
        .unwrap()
        .to_der()
        .unwrap();

    (
        cert,
        native_tls::Identity::from_pkcs12(&encrypted, &password).expect("Unable to build identity"),
    )
}

fn start_proxy<'a>(proxy: &mut Proxy) {
    let mut state = STATE.lock().unwrap();

    if proxy.started {
        panic!("Tried to start an already started proxy");
    }
    proxy.started = true;
    let mocks = proxy.mocks.clone();

    // if state.listening_addr.is_some() {
    //     return;
    // }

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let res = TcpListener::bind(SERVER_ADDRESS_INTERNAL).or_else(|err| {
            error!("TcpListener::bind: {}", err);
            TcpListener::bind("127.0.0.1:0")
        });
        let (listener, addr) = match res {
            Ok(listener) => {
                let addr = listener.local_addr().unwrap();
                tx.send(Some(addr)).unwrap();
                (listener, addr)
            }
            Err(err) => {
                error!("alt bind: {}", err);
                tx.send(None).unwrap();
                return;
            }
        };

        info!("Server is listening at {}", addr);
        for stream in listener.incoming() {
            info!("Got stream: {:?}", stream);
            if let Ok(mut stream) = stream {
                let request = Request::from(Box::new(&mut stream));
                info!("Request received: {}", request);
                if request.is_ok() {
                    handle_request(&mocks, request, stream).unwrap();
                } else {
                    let message = request
                        .error()
                        .map_or("Could not parse the request.", |err| err.as_str());
                    error!("Could not parse request because: {}", message);
                    respond_with_error(stream, request.version, message);
                }
            } else {
                error!("Could not read from stream");
            }
        }
    });

    state.listening_addr = rx.recv().ok().and_then(|addr| addr);
    proxy.listening_addr = state.listening_addr.clone();
}

fn open_tunnel(
    request: Request,
    stream: &mut TcpStream,
) -> Result<TlsStream<&mut TcpStream>, Box<dyn std::error::Error>> {
    let version = request.version;
    let status = 200;

    let response = Vec::from(format!(
        "HTTP/{}.{} {}\r\n\r\n",
        version.0, version.1, status
    ));

    stream.write_all(&response)?;
    stream.flush()?;
    info!("Response written");

    let identity = STATE.lock().unwrap().identity.clone();

    info!("Wrapping with tls");
    let tstream = native_tls::TlsAcceptor::builder(identity)
        .build()
        .expect("Unable to build acceptor")
        .accept(stream)
        .expect("Unable to accept connection");
    info!("Wrapped: {:?}", tstream);

    Ok(tstream)
}

fn handle_request(
    mocks: &Vec<Mock>,
    request: Request,
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tstream = open_tunnel(request, &mut stream)?;

    let req = Request::from(Box::new(&mut tstream));

    for m in mocks {
        if m.matches(&req) {
            write_response(&mut tstream, &req, &m.response)?;
            break;
        }
    }

    Ok(())
}

fn write_response(
    tstream: &mut TlsStream<&mut TcpStream>,
    request: &Request,
    response: &Response,
) -> Result<(), Box<dyn std::error::Error>> {
    tstream.write_fmt(format_args!(
        "HTTP/1.{} {}\r\n",
        request.version.1,
        response.status
    ))?;
    for (header, value) in &response.headers {
        tstream.write_fmt(format_args!("{}: {}\r\n", header, value))?;
    }
    tstream.write(b"\r\n")?;
    tstream.write_all(&response.body)?;
    tstream.write(b"\r\n")?;

    Ok(())
}

fn respond_with_error(stream: TcpStream, version: (u8, u8), message: &str) {}
