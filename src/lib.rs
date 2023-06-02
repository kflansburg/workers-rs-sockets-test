use std::{future::Future, pin::Pin};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wasm_bindgen::prelude::wasm_bindgen;
use worker::{console_log, event, Context, Env, Request, Response, SecureTransport, Socket};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen]
    fn setTimeout(funcRef: js_sys::Function, delay: js_sys::Number);
}

async fn timeout(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        setTimeout(resolve, ms.into());
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.ok();
}

async fn test_no_ssl() -> Result<(), String> {
    let mut socket = Socket::builder()
        .secure_transport(SecureTransport::Off)
        .connect("example.com", 80)
        .map_err(|e| format!("connect failed: {:?}", e))?;

    socket
        .write(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
        .await
        .map_err(|e| format!("socket.write failed: {:?}", e))?;

    let mut buf = Vec::new();
    socket
        .read_to_end(&mut buf)
        .await
        .map_err(|e| format!("socket.read_to_end failed: {:?}", e))?;

    Ok(())
}

async fn test_ssl() -> Result<(), String> {
    let mut socket = Socket::builder()
        .secure_transport(SecureTransport::On)
        .connect("example.com", 443)
        .map_err(|e| format!("connect failed: {:?}", e))?;

    socket
        .write(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
        .await
        .map_err(|e| format!("socket.write failed: {:?}", e))?;

    let mut buf = Vec::new();
    socket
        .read_to_end(&mut buf)
        .await
        .map_err(|e| format!("socket.read_to_end failed: {:?}", e))?;

    Ok(())
}

async fn test_start_tls() -> Result<(), String> {
    let plaintext = Socket::builder()
        .secure_transport(SecureTransport::StartTls)
        .connect("example.com", 443)
        .map_err(|e| format!("connect failed: {:?}", e))?;

    let mut socket = plaintext.start_tls();

    socket
        .write(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
        .await
        .map_err(|e| format!("socket.write failed: {:?}", e))?;

    let mut buf = Vec::new();
    socket
        .read_to_end(&mut buf)
        .await
        .map_err(|e| format!("socket.read_to_end failed: {:?}", e))?;

    Ok(())
}

async fn test_allow_half_open() -> Result<(), String> {
    let mut socket = Socket::builder()
        .allow_half_open(true)
        .connect("example.com", 443)
        .map_err(|e| format!("connect failed: {:?}", e))?;

    socket
        .write(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
        .await
        .map_err(|e| format!("socket.write failed: {:?}", e))?;

    let mut buf = Vec::new();
    socket
        .read_to_end(&mut buf)
        .await
        .map_err(|e| format!("socket.read_to_end failed: {:?}", e))?;

    // Note, this is tricky to test because most HTTP servers either write EOF and disconnect or
    // write no EOF and stay connected. So we can only verify that we don't hit our own "writer closed"
    // error and instead encounter a connection closed error.
    match socket.write(b"FOO").await {
        Ok(_) => Ok(()),
        Err(e) => match e.get_ref().ok_or("std::io::Error no inner")? {
            e if e.to_string() == "Error: Network connection lost." => Ok(()),
            e => Err(format!("Unexpected error: {:?}", e)),
        },
    }
}

async fn test_disallow_half_open() -> Result<(), String> {
    let mut socket = Socket::builder()
        .allow_half_open(false)
        .connect("example.com", 443)
        .map_err(|e| format!("connect failed: {:?}", e))?;

    socket
        .write(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
        .await
        .map_err(|e| format!("socket.write failed: {:?}", e))?;

    let mut buf = Vec::new();
    socket
        .read_to_end(&mut buf)
        .await
        .map_err(|e| format!("socket.read_to_end failed: {:?}", e))?;

    match socket.write(b"FOO").await {
        Ok(_) => Err("Write after EOF succeeded.".to_string()),
        Err(e) => match e.get_ref().ok_or("std::io::Error no inner")? {
            e if e.to_string() == "TypeError: This WritableStream has been closed." => Ok(()),
            e => Err(format!("Unexpected error: {:?}", e)),
        },
    }
}

type Test = Pin<Box<dyn Future<Output = Result<(), String>>>>;

#[event(fetch)]
async fn main(_req: Request, _env: Env, _ctx: Context) -> worker::Result<Response> {
    let tests: Vec<(&str, Test)> = vec![
        ("NO_SSL", Box::pin(test_no_ssl())),
        ("SSL", Box::pin(test_ssl())),
        ("StartTls", Box::pin(test_start_tls())),
        ("ALLOW_HALF_OPEN", Box::pin(test_allow_half_open())),
        ("DISALLOW_HALF_OPEN", Box::pin(test_disallow_half_open())),
    ];

    let mut failed = false;
    let mut test_results = Vec::with_capacity(tests.len());

    for (name, fut) in tests {
        console_log!("Running Test {}", name);
        let result = tokio::select! {
            res = fut => match res {
                Ok(()) => format!("[SUCCESS] {}", name),
                Err(error) => {
                    failed = true;
                    format!("[FAILED] {}: {}", name, error)
                }
            },
            _ = timeout(5_000) => {
                failed = true;
                format!("[FAILED] {}: Timed out!", name)
            }
        };
        console_log!("{}", &result);
        test_results.push(result);
    }
    let body = test_results.join("\n");

    if failed {
        Response::error(body, 500)
    } else {
        Response::ok(body)
    }
}
