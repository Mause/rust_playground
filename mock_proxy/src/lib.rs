//! This library was built to help test systems that use libraries which don't provide any
//! testing utilities themselves. It works by overriding the proxy and root ca attributes
//! and intercepting proxy requests, then returning mock responses defined by the user

use crate::mock::Response;
use log::{error, info};
use native_tls::TlsStream;
use openssl::pkey::{PKey, PKeyRef, Private};
use openssl::x509::X509Ref;
use std::io::{Read, Write as IOWrite};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

mod identity;
mod mock;
pub use crate::mock::Mock;

const SERVER_ADDRESS_INTERNAL: &str = "127.0.0.1:1234";

/// Primary interface for the library
pub struct Proxy {
    mocks: Vec<Mock>,
    listening_addr: Option<SocketAddr>,
    started: bool,
    identity: PKey<Private>,
    cert: openssl::x509::X509,
}

impl Default for Proxy {
    fn default() -> Self {
        let (cert, identity) = crate::identity::mk_ca_cert().unwrap();
        Self {
            mocks: Vec::new(),
            listening_addr: None,
            started: false,
            identity,
            cert,
        }
    }
}

struct Pair<'a>(&'a X509Ref, &'a PKeyRef<Private>);

impl Proxy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a given mock with the proxy
    ///
    /// Will panic if proxy has already been started
    pub fn register(&mut self, mock: Mock) {
        if self.started {
            panic!("Cannot add mocks to a started proxy");
        }
        self.mocks.push(mock);
    }

    /// Start the proxy server
    ///
    /// Will panic if proxy has already been started
    pub fn start(&mut self) {
        start_proxy(self);
    }

    /// Start the server
    pub fn stop(&mut self) {
        todo!();
    }

    /// Address and port of the local server.
    /// Can be used with `std::net::TcpStream`.
    pub fn address(&self) -> SocketAddr {
        self.listening_addr.expect("server should be listening")
    }

    /// A local `http://…` URL of the server.
    pub fn url(&self) -> String {
        format!("http://{}", self.address())
    }

    /// Returns the root CA certificate of the server
    pub fn get_certificate(&self) -> Vec<u8> {
        self.cert.to_pem().unwrap().clone()
    }
}

#[derive(Debug, Clone)]
struct Request {
    error: Option<String>,
    path: Option<String>,
    method: Option<String>,
    version: (u8, u8),
}

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path)
            .finish_non_exhaustive()
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
                httparse::Status::Complete(_head_length) => {
                    request.method = req.method.map(|s| s.to_string());
                    request.path = req
                        .path
                        .map(|s| s.to_string().split(":").next().unwrap().to_owned());
                    if let Some(a @ 0..=1) = req.version {
                        request.version = (1, a);
                    }
                }
                httparse::Status::Partial => panic!("Incomplete request"),
            });

        request
    }
}

fn create_identity(cn: &str, pair: Pair) -> native_tls::Identity {
    let (cert, key) = crate::identity::mk_ca_signed_cert(cn, pair.0, pair.1).unwrap();

    let password = "password";
    let encrypted = openssl::pkcs12::Pkcs12::builder()
        .build(password, cn, &key, &cert)
        .unwrap()
        .to_der()
        .unwrap();

    native_tls::Identity::from_pkcs12(&encrypted, &password).expect("Unable to build identity")
}

fn start_proxy<'a>(proxy: &mut Proxy) {
    if proxy.started {
        panic!("Tried to start an already started proxy");
    }
    proxy.started = true;
    let mocks = proxy.mocks.clone();
    let cert = proxy.cert.clone();
    let pkey = proxy.identity.clone();

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
                    handle_request(Pair(cert.as_ref(), pkey.as_ref()), &mocks, request, stream)
                        .unwrap();
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

    proxy.listening_addr = rx.recv().ok().and_then(|addr| addr);
}

fn open_tunnel<'a>(
    identity: Pair,
    request: Request,
    stream: &'a mut TcpStream,
) -> Result<TlsStream<&'a mut TcpStream>, Box<dyn std::error::Error>> {
    let version = request.version;
    let status = 200;

    let response = Vec::from(format!(
        "HTTP/{}.{} {}\r\n\r\n",
        version.0, version.1, status
    ));

    stream.write_all(&response)?;
    stream.flush()?;
    info!("Response written");

    let identity = create_identity(&request.path.unwrap(), identity);

    info!("Wrapping with tls");
    let tstream = native_tls::TlsAcceptor::builder(identity.clone())
        .build()
        .expect("Unable to build acceptor")
        .accept(stream)
        .expect("Unable to accept connection");
    info!("Wrapped: {:?}", tstream);

    Ok(tstream)
}

fn handle_request(
    identity: Pair,
    mocks: &Vec<Mock>,
    request: Request,
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tstream = open_tunnel(identity, request, &mut stream)?;

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
        request.version.1, response.status
    ))?;
    for (header, value) in &response.headers {
        tstream.write_fmt(format_args!("{}: {}\r\n", header, value))?;
    }
    tstream.write(b"\r\n")?;
    tstream.write_all(&response.body)?;
    tstream.write(b"\r\n")?;

    Ok(())
}

fn respond_with_error(_stream: TcpStream, _version: (u8, u8), _message: &str) {
    todo!();
}
