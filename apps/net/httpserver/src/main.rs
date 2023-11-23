//! Simple HTTP server.
//!
//! Benchmark with [Apache HTTP server benchmarking tool](https://httpd.apache.org/docs/2.4/programs/ab.html):
//!
//! ```
//! ab -n 5000 -c 20 http://X.X.X.X:5555/
//! ```

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use std::string::String;
use std::vec::Vec;
use std::io::{self, prelude::*};
use std::net::{TcpListener, TcpStream};
use std::thread;

const LOCAL_IP: &str = "0.0.0.0";
const LOCAL_PORT: u16 = 5555;

macro_rules! header {
    () => {
        "\
HTTP/1.1 200 OK\r\n\
Content-Type: text/html\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
\r\n\
{}"
    };
}

macro_rules! image_header {
    () => {
        "\
HTTP/1.1 200 OK\r\n\
Content-Type: image/png\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
\r\n\
"
    };
}

macro_rules! info {
    ($($arg:tt)*) => {
        match option_env!("LOG") {
            Some("info") | Some("debug") | Some("trace") => {
                print!("[INFO] {}\n", format_args!($($arg)*));
            }
            _ => {}
        }
    };
}

fn http_server(mut stream: TcpStream) -> io::Result<()> {
    let mut buf = [0u8; 4096];
    let _len = stream.read(&mut buf)?;

    let request_head = buf.split(|&x| x == b'\n').next().unwrap();
    let mut iter = request_head.split(|&x| x == b' ');

    let method = iter.next().unwrap();
    let path = iter.next().unwrap();
    let version = iter.next().unwrap();

    let mut path = std::str::from_utf8(path).unwrap().trim();

    if path == "/" {
        path = "/index.html";
    }

    match path.split(".").last().unwrap() {
        "html" => {
            let content = std::fs::read(format!("/html{}", path).as_str()).unwrap();
            let response = format!(
                header!(),
                content.len(),
                std::str::from_utf8(&content).unwrap()
            );

            stream.write_all(response.as_bytes())?;
        }
        "png" | "jpg" => {
            let content = std::fs::read(format!("/png{}", path).as_str()).unwrap();
            let response_header = format!(
                image_header!(),
                content.len(),
            );
            stream.write_all(response_header.as_bytes())?;
            stream.write_all(&content)?;
        }
        _ => {}
    }
    stream.flush()?;

    Ok(())
}

fn accept_loop() -> io::Result<()> {
    let listener = TcpListener::bind((LOCAL_IP, LOCAL_PORT))?;
    println!("listen on: http://{}/", listener.local_addr().unwrap());

    let mut i = 0;
    loop {
        match listener.accept() {
            Ok((stream, addr)) => {
                info!("new client {}: {}", i, addr);
                thread::spawn(move || match http_server(stream) {
                    Err(e) => info!("client connection error: {:?}", e),
                    Ok(()) => info!("client {} closed successfully", i),
                });
            }
            Err(e) => return Err(e),
        }
        i += 1;
    }
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    println!("Hello, ArceOS HTTP server!");
    accept_loop().expect("test HTTP server failed");
}
