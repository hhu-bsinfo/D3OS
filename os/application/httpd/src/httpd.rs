//! ping – answer HTTP requests
#![no_std]
extern crate alloc;

use core::{fmt::Display, net::{IpAddr, Ipv6Addr, SocketAddr}};

use alloc::{collections::btree_map::BTreeMap, format, string::String, vec::Vec};
use concurrent::thread;
use httparse::{EMPTY_HEADER, Request};
use naming::{open, read, shared_types::OpenOptions};
use network::{NetworkError, TcpListener, TcpStream};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

#[unsafe(no_mangle)]
fn main() {
    // ignore all args for now
    let port = 1797;
    let ip = IpAddr::V6(Ipv6Addr::UNSPECIFIED);
    let webroot = "/usr/www";
    println!("serving {} on [{}]:{}", webroot, ip, port);
    
    let mut listener = TcpListener::bind(SocketAddr::new(ip, port))
        .expect("failed to bind socket");
    loop {
        // TODO: if we get many concurrent requests, they may fail
        // this is probably because we have no backlog
        if let Ok(client) = listener.accept() {
            thread::create(|| {
                println!("got a connection from {}", client.peer_addr());
                let mut buffer: [u8; 4096] = [0; 4096];
                if let Ok(len) = client.read(&mut buffer) {
                    let mut headers = [EMPTY_HEADER; 64];
                    let mut request = Request::new(&mut headers);
                    match request.parse(&buffer[0..len]) {
                        Ok(_body_start) => if let Err(e) = handle(request, webroot).send_to(client) {
                            println!("couldn't send reponse to client: {:?}", e);
                        },
                        Err(e) => println!("couldn't parse client request: {:?}", e),
                    }
                }
            });
        }
    }
}

struct Response {
    status: StatusCode,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl Response {
    /// Create a new response.
    fn new(status: StatusCode) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert("Server".into(), "D3OS httpd".into());
        headers.insert("Connection".into(), "close".into());
        Self { status, headers, body: Vec::new() }
    }
    
    /// Send this response to a client.
    fn send_to(mut self, client: TcpStream) -> Result<(),  NetworkError> {
        client.write(format!("HTTP/1.1 {}\n", self.status).as_bytes())?;
        self.headers.insert("Content-Length".into(), format!("{}", self.body.len()).into());
        for (header_name, header_value) in self.headers {
            client.write(format!("{}: {}\n", header_name, header_value).as_bytes())?;
        }
        client.write(b"\n")?;
        // we seem to be able to send about 64k at a time, but why?
        let mut sent = 0;
        loop {
            let now_sent = client.write(&self.body[sent..])?;
            sent += now_sent;
            //println!("sent {} bytes", sent);
            if sent == self.body.len() || now_sent == 0 {
                break;
            }
        }
        client.write(b"\n\n")?;
        Ok(())
    }
}

/// HTTP Status Codes as in <https://datatracker.ietf.org/doc/html/rfc9110#name-status-codes>
enum StatusCode {
    Ok,
    NotFound,
    MethodNotAllowed,
}

impl Display for StatusCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            Self::Ok => "200 OK",
            Self::NotFound => "404 Not Found",
            Self::MethodNotAllowed => "405 Method Not Allowed",
        })
    }
}

/// Handle this request, returning a response.
fn handle(request: Request, webroot: &str) -> Response {
    println!("handling request {:?}", request);
    if request.method == Some("GET") {
        if let Some(mut request_path) = request.path {
            request_path = &request_path[1..]; // strip the first /
            if request_path.is_empty() {
                request_path = "index.html";
            }
            let path = format!("{webroot}/{request_path}");
            if let Ok(fh) = open(&path, OpenOptions::READONLY) {
                let mut r = Response::new(StatusCode::Ok);
                let mut buf: [u8; 4096] = [0; 4096];
                loop {
                    let amount = read(fh, &mut buf).expect("failed to read file");
                    //println!("read {} of {}", amount, request_path);
                    if amount == 0 {
                        break;
                    }
                    r.body.extend(&buf[0..amount]);
                }
                r.headers.insert("Content-Type".into(), match infer::get(&r.body) {
                    Some(t) => t.mime_type(),
                    None => {
                        println!("failed to get mime type for {}, trying to figure it out from the extension", request_path);
                        match request_path.rsplit_once(".") {
                            Some((_, "html")) => "text/html",
                            Some((_, "css")) => "text/css",
                            Some((_, "js")) => "application/javascript",
                            _ => "text/plain; charset=UTF-8",
                        }
                    },
                }.into());
                return r;
            }
        }
        let mut r = Response::new(StatusCode::NotFound);
        r.headers.insert("Content-Type".into(), "text/plain; charset=UTF-8".into());
        r.body.extend("File not found".as_bytes());
        r
    } else {
        let mut r = Response::new(StatusCode::MethodNotAllowed);
        r.headers.insert("Allowed".into(), "GET".into());
        r.body.extend(format!("Method not allowed").as_bytes());
        r
    }
}
