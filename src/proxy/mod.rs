use crate::proxy::mock::{Mock, Response};
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

lazy_static! {
    pub static ref STATE: Mutex<State> = Mutex::new(State::new());
}
const SERVER_ADDRESS_INTERNAL: &str = "127.0.0.1:1234";

pub struct Proxy<'a> {
    mocks: Vec<&'a mockito::Mock>,
    listening_addr: Option<SocketAddr>,
}

impl<'a> Default for Proxy<'a> {
    fn default() -> Self {
        Self {
            mocks: Vec::new(),
            listening_addr: None,
        }
    }
}

impl<'a> Proxy<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, mock: &'a mockito::Mock) {
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

struct Request {
    error: Option<String>,
    host: Option<String>,
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
}

impl<'a> From<&'a TcpStream> for Request {
    fn from(mut stream: &TcpStream) -> Self {
        let mut request = Self {
            error: None,
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
                    println!("Req: {} {:?}", head_length, req);
                    request.host = req.path.map(|s| s.to_string());
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

    // if state.listening_addr.is_some() {
    //     return;
    // }

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let res = TcpListener::bind(SERVER_ADDRESS_INTERNAL).or_else(|err| {
            println!("TcpListener::bind: {}", err);
            TcpListener::bind("127.0.0.1:0")
        });
        let (listener, addr) = match res {
            Ok(listener) => {
                let addr = listener.local_addr().unwrap();
                tx.send(Some(addr)).unwrap();
                (listener, addr)
            }
            Err(err) => {
                println!("alt bind: {}", err);
                tx.send(None).unwrap();
                return;
            }
        };

        info!("Server is listening at {}", addr);
        for stream in listener.incoming() {
            info!("Got stream: {:?}", stream);
            if let Ok(stream) = stream {
                let request = Request::from(&stream);
                info!("Request received: {}", request);
                if request.is_ok() {
                    handle_request(request, stream).unwrap();
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
    request: Request,
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tstream = open_tunnel(request, &mut stream)?;

    let mut all_buf = Vec::new();
    loop {
        let mut buf = [0; 1024];

        let rlen = match tstream.read(&mut buf) {
            Err(e) => Err(e.to_string()),
            Ok(0) => Err("Nothing to read.".into()),
            Ok(i) => Ok(i),
        }
        .unwrap();

        all_buf.extend_from_slice(&buf[..rlen]);

        if rlen < 1024 {
            break;
        }
    }
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);

    let result = req.parse(&all_buf).unwrap();
    println!("{:?} {:?}", result, req);

    let mut m = Mock::new("POST", "/api/v8/channels/0/messages");

    m.response.body = std::fs::read("src/message.json").unwrap();

    if m.path == req.path.unwrap() && m.method == req.method.unwrap() {
        write_response(&mut tstream, req, m.response)?;
    }

    Ok(())
}

fn write_response(
    tstream: &mut TlsStream<&mut TcpStream>,
    request: httparse::Request,
    response: Response,
) -> Result<(), Box<dyn std::error::Error>> {
    tstream.write_fmt(format_args!(
        "HTTP/1.{} {}\r\n",
        request.version.expect("version"),
        response.status
    ))?;
    for (header, value) in response.headers {
        tstream.write_fmt(format_args!("{}: {}\r\n", header, value))?;
    }
    tstream.write(b"\r\n")?;
    tstream.write_all(&response.body)?;
    tstream.write(b"\r\n")?;

    Ok(())
}

fn respond_with_error(stream: TcpStream, version: (u8, u8), message: &str) {}
