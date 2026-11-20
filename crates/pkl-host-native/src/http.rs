//! Pure safe Rust HTTP/1.1 + `rustls` HTTPS + HTTP/CONNECT proxy transport implementing
//! `pkl_runtime::host::http::HttpTransport` (single-hop, NO redirect following) with
//! cooperative cancellation & deadline enforcement.

use crate::certs::CaCertSource;
use pkl_runtime::host::http::{
    HttpClientConfig, HttpError, HttpRequest, HttpResponse, HttpTransport, LazyHttpTransport,
    ProxyAddress, RewritingHttpClient,
};
use pkl_runtime::value::PklString;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const POLL_SLICE: Duration = Duration::from_millis(50);

/// Shared cancellation flag checked across socket connects and read/write slices.
#[derive(Debug, Default)]
pub struct CancelFlag {
    cancelled: AtomicBool,
}

impl CancelFlag {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cancelled: AtomicBool::new(false),
        })
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Single-hop non-redirecting native HTTP/HTTPS transport (`JdkHttpClient` equivalent).
pub struct NativeHttpTransport {
    tls_config: Arc<ClientConfig>,
    cancel_flag: Arc<CancelFlag>,
    closed: AtomicBool,
}

impl NativeHttpTransport {
    pub fn new(tls_config: Arc<ClientConfig>, cancel_flag: Arc<CancelFlag>) -> Self {
        Self {
            tls_config,
            cancel_flag,
            closed: AtomicBool::new(false),
        }
    }

    fn check_aborted(&self, deadline: Instant) -> Result<(), HttpError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(HttpError::Closed("HTTP client has been closed.".into()));
        }
        if self.cancel_flag.is_cancelled() {
            return Err(HttpError::Io("HTTP request cancelled.".into()));
        }
        if Instant::now() >= deadline {
            return Err(HttpError::Io("HTTP request timed out.".into()));
        }
        Ok(())
    }

    fn connect_tcp(
        &self,
        host: &str,
        port: u16,
        connect_timeout: Duration,
        overall_deadline: Instant,
    ) -> Result<TcpStream, HttpError> {
        self.check_aborted(overall_deadline)?;
        let addrs: Vec<SocketAddr> = (host, port)
            .to_socket_addrs()
            .map_err(|_| HttpError::Io(format!("Error connecting to host `{host}`.").into()))?
            .collect();
        if addrs.is_empty() {
            return Err(HttpError::Io(
                format!("Error connecting to host `{host}`.").into(),
            ));
        }

        let connect_deadline = Instant::now()
            .checked_add(connect_timeout)
            .unwrap_or(overall_deadline)
            .min(overall_deadline);

        let mut last_err = None;
        for addr in addrs {
            self.check_aborted(overall_deadline)?;
            let remaining = connect_deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match TcpStream::connect_timeout(&addr, remaining) {
                Ok(stream) => {
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_read_timeout(Some(POLL_SLICE));
                    let _ = stream.set_write_timeout(Some(POLL_SLICE));
                    return Ok(stream);
                }
                Err(err) => {
                    last_err = Some(err);
                }
            }
        }
        let _ = last_err;
        Err(HttpError::Io(
            format!("Error connecting to host `{host}`.").into(),
        ))
    }

    fn open_proxy_connect_tunnel(
        &self,
        proxy: &ProxyAddress,
        target_raw_host: &str,
        target_port: u16,
        connect_timeout: Duration,
        deadline: Instant,
    ) -> Result<TcpStream, HttpError> {
        let mut stream = self.connect_tcp(&proxy.host, proxy.port, connect_timeout, deadline)?;
        let authority = format!("{target_raw_host}:{target_port}");
        let req = format!(
            "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n\r\n"
        );
        write_all_cancellable(
            &mut stream,
            req.as_bytes(),
            self,
            deadline,
            &proxy.host,
            false,
        )?;

        // Read response status line and headers up to `\r\n\r\n`.
        let mut header_bytes = Vec::with_capacity(256);
        let mut byte = [0u8; 1];
        loop {
            let n =
                read_some_cancellable(&mut stream, &mut byte, self, deadline, &proxy.host, false)?;
            if n == 0 {
                return Err(HttpError::Io(
                    "Proxy closed connection during CONNECT tunnel setup.".into(),
                ));
            }
            header_bytes.push(byte[0]);
            if header_bytes.ends_with(b"\r\n\r\n") {
                break;
            }
            if header_bytes.len() > 64 * 1024 {
                return Err(HttpError::Io(
                    "Proxy CONNECT response headers too large.".into(),
                ));
            }
        }
        let header_text = String::from_utf8_lossy(&header_bytes);
        let status_line = header_text.lines().next().unwrap_or("");
        let status_code = parse_status_code(status_line).unwrap_or(0);
        if !(200..300).contains(&status_code) {
            return Err(HttpError::Io(
                format!("HTTP proxy CONNECT tunnel failed with status `{status_code}`.").into(),
            ));
        }
        Ok(stream)
    }

    fn send_over_stream<S: Read + Write>(
        &self,
        stream: &mut S,
        request: &HttpRequest,
        use_absolute_form: bool,
        deadline: Instant,
    ) -> Result<HttpResponse, HttpError> {
        let target = if use_absolute_form {
            request.parsed.absolute_form_target()
        } else {
            request.parsed.request_target()
        };
        let mut wire = format!(
            "GET {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            request.parsed.host_header()
        );
        for (name, value) in &request.headers {
            wire.push_str(name);
            wire.push_str(": ");
            wire.push_str(value);
            wire.push_str("\r\n");
        }
        wire.push_str("\r\n");

        let is_https = request.parsed.scheme == "https";
        write_all_cancellable(
            stream,
            wire.as_bytes(),
            self,
            deadline,
            &request.parsed.host,
            is_https,
        )?;
        flush_cancellable(stream, self, deadline, &request.parsed.host, is_https)?;

        read_http_response(stream, self, deadline, &request.parsed.host, is_https)
    }
}

impl HttpTransport for NativeHttpTransport {
    fn send_single_hop(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let connect_timeout = Duration::from_millis(request.connect_timeout_ms.max(1));
        let request_timeout = Duration::from_millis(request.request_timeout_ms.max(1));
        let deadline = Instant::now()
            .checked_add(request_timeout)
            .unwrap_or_else(Instant::now);

        self.check_aborted(deadline)?;

        let is_https = request.parsed.scheme == "https";
        let target_port = request.parsed.effective_port();

        if is_https {
            let tcp = match &request.proxy {
                Some(proxy) => self.open_proxy_connect_tunnel(
                    proxy,
                    &request.parsed.raw_host,
                    target_port,
                    connect_timeout,
                    deadline,
                )?,
                None => {
                    self.connect_tcp(&request.parsed.host, target_port, connect_timeout, deadline)?
                }
            };

            let server_name = ServerName::try_from(request.parsed.host.clone()).map_err(|err| {
                HttpError::Io(
                    format!(
                        "Error during SSL handshake with host `{}`:\n{err}",
                        request.parsed.host
                    )
                    .into(),
                )
            })?;
            let conn =
                ClientConnection::new(self.tls_config.clone(), server_name).map_err(|err| {
                    HttpError::Io(
                        format!(
                            "Error during SSL handshake with host `{}`:\n{err}",
                            request.parsed.host
                        )
                        .into(),
                    )
                })?;
            let mut tls_stream = StreamOwned::new(conn, tcp);
            complete_tls_handshake(&mut tls_stream, self, deadline, &request.parsed.host)?;
            self.send_over_stream(&mut tls_stream, request, false, deadline)
        } else {
            let (mut tcp, use_absolute_form) = match &request.proxy {
                Some(proxy) => (
                    self.connect_tcp(&proxy.host, proxy.port, connect_timeout, deadline)?,
                    true,
                ),
                None => (
                    self.connect_tcp(&request.parsed.host, target_port, connect_timeout, deadline)?,
                    false,
                ),
            };
            self.send_over_stream(&mut tcp, request, use_absolute_form, deadline)
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.cancel_flag.cancel();
    }
}

fn complete_tls_handshake(
    stream: &mut StreamOwned<ClientConnection, TcpStream>,
    transport: &NativeHttpTransport,
    deadline: Instant,
    host: &str,
) -> Result<(), HttpError> {
    while stream.conn.is_handshaking() {
        transport.check_aborted(deadline)?;
        match stream.conn.complete_io(&mut stream.sock) {
            Ok(_) => {}
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(err) => {
                let cause = format_ssl_handshake_cause(&err);
                return Err(HttpError::Io(
                    format!("Error during SSL handshake with host `{host}`:\n{cause}").into(),
                ));
            }
        }
    }
    Ok(())
}

fn write_all_cancellable<W: Write>(
    writer: &mut W,
    mut buf: &[u8],
    transport: &NativeHttpTransport,
    deadline: Instant,
    host: &str,
    is_https: bool,
) -> Result<(), HttpError> {
    while !buf.is_empty() {
        transport.check_aborted(deadline)?;
        match writer.write(buf) {
            Ok(0) => {
                return Err(HttpError::Io(
                    "Connection closed while writing HTTP request".into(),
                ));
            }
            Ok(n) => buf = &buf[n..],
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(err) => return Err(classify_io_error(&err, host, is_https)),
        }
    }
    Ok(())
}

fn flush_cancellable<W: Write>(
    writer: &mut W,
    transport: &NativeHttpTransport,
    deadline: Instant,
    host: &str,
    is_https: bool,
) -> Result<(), HttpError> {
    loop {
        transport.check_aborted(deadline)?;
        match writer.flush() {
            Ok(()) => return Ok(()),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(err) => return Err(classify_io_error(&err, host, is_https)),
        }
    }
}

fn read_some_cancellable<R: Read>(
    reader: &mut R,
    buf: &mut [u8],
    transport: &NativeHttpTransport,
    deadline: Instant,
    host: &str,
    is_https: bool,
) -> Result<usize, HttpError> {
    loop {
        transport.check_aborted(deadline)?;
        match reader.read(buf) {
            Ok(n) => return Ok(n),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(err) => return Err(classify_io_error(&err, host, is_https)),
        }
    }
}

struct CancellableReader<'a, R: Read> {
    inner: &'a mut R,
    transport: &'a NativeHttpTransport,
    deadline: Instant,
    aborted: Option<HttpError>,
}

impl<R: Read> Read for CancellableReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if let Err(http_err) = self.transport.check_aborted(self.deadline) {
                self.aborted = Some(http_err);
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "aborted"));
            }
            match self.inner.read(buf) {
                Ok(n) => return Ok(n),
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(err) => return Err(err),
            }
        }
    }
}

fn map_reader_error<R: Read>(
    reader: &mut BufReader<CancellableReader<'_, R>>,
    err: &io::Error,
    host: &str,
    is_https: bool,
) -> HttpError {
    if let Some(aborted) = reader.get_mut().aborted.take() {
        return aborted;
    }
    classify_io_error(err, host, is_https)
}

fn read_http_response<R: Read>(
    stream: &mut R,
    transport: &NativeHttpTransport,
    deadline: Instant,
    host: &str,
    is_https: bool,
) -> Result<HttpResponse, HttpError> {
    let mut reader = BufReader::new(CancellableReader {
        inner: stream,
        transport,
        deadline,
        aborted: None,
    });

    let mut status_line = String::new();
    let n = match reader.read_line(&mut status_line) {
        Ok(n) => n,
        Err(err) => return Err(map_reader_error(&mut reader, &err, host, is_https)),
    };
    if n == 0 {
        return Err(HttpError::Io(
            "Unexpected EOF reading HTTP status line.".into(),
        ));
    }
    let status = parse_status_code(&status_line).ok_or_else(|| {
        HttpError::Io(format!("Malformed HTTP status line: `{}`", status_line.trim()).into())
    })?;

    let mut headers = Vec::new();
    let mut content_length: Option<usize> = None;
    let mut is_chunked = false;

    loop {
        let mut line = String::new();
        let n = match reader.read_line(&mut line) {
            Ok(n) => n,
            Err(err) => return Err(map_reader_error(&mut reader, &err, host, is_https)),
        };
        if n == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            let name = name.trim().to_string();
            let value = value.trim().to_string();
            if name.eq_ignore_ascii_case("content-length")
                && let Ok(len) = value.parse::<usize>()
            {
                content_length = Some(len);
            }
            if name.eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
            {
                is_chunked = true;
            }
            headers.push((name, value));
        }
    }

    let body = if matches!(status, 204 | 304) || (100..200).contains(&status) {
        Vec::new()
    } else if is_chunked {
        match read_chunked_body(&mut reader) {
            Ok(b) => b,
            Err(err) => return Err(map_reader_error(&mut reader, &err, host, is_https)),
        }
    } else if let Some(len) = content_length {
        let mut buf = vec![0u8; len];
        if let Err(err) = reader.read_exact(&mut buf) {
            return Err(map_reader_error(&mut reader, &err, host, is_https));
        }
        buf
    } else {
        let mut buf = Vec::new();
        match reader.read_to_end(&mut buf) {
            Ok(_) => {}
            // Treat TLS close_notify omission on EOF cleanly when peer closed after body.
            Err(err)
                if reader.get_ref().aborted.is_none()
                    && is_https
                    && err.kind() == io::ErrorKind::UnexpectedEof => {}
            Err(err) => return Err(map_reader_error(&mut reader, &err, host, is_https)),
        }
        buf
    };

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn read_chunked_body<R: BufRead>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let mut size_line = String::new();
        let n = reader.read_line(&mut size_line)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Unexpected EOF reading chunk size",
            ));
        }
        let size_str = size_line.trim().split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(size_str, 16).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid HTTP chunk size `{size_str}`"),
            )
        })?;
        if chunk_size == 0 {
            // Drain trailer lines up to empty line.
            loop {
                let mut trailer = String::new();
                let m = reader.read_line(&mut trailer)?;
                if m == 0 || trailer.trim_end_matches(['\r', '\n']).is_empty() {
                    break;
                }
            }
            break;
        }
        let start = out.len();
        out.resize(start + chunk_size, 0);
        reader.read_exact(&mut out[start..])?;
        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;
    }
    Ok(out)
}

fn parse_status_code(line: &str) -> Option<u16> {
    let mut parts = line.split_ascii_whitespace();
    let proto = parts.next()?;
    if !proto.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse::<u16>().ok()
}

fn format_ssl_handshake_cause(err: &io::Error) -> String {
    if let Some(rustls_err) = err
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        && matches!(
            rustls_err,
            rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer)
        )
    {
        return "unable to find valid certification path to requested target".to_string();
    }
    let msg = err.to_string();
    if msg.contains("UnknownIssuer") {
        "unable to find valid certification path to requested target".to_string()
    } else {
        msg
    }
}

fn classify_io_error(err: &io::Error, host: &str, is_https: bool) -> HttpError {
    if is_https && err.kind() == io::ErrorKind::InvalidData {
        let cause = format_ssl_handshake_cause(err);
        return HttpError::Io(
            format!("Error during SSL handshake with host `{host}`:\n{cause}").into(),
        );
    }
    if matches!(
        err.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::HostUnreachable
            | io::ErrorKind::NetworkUnreachable
    ) {
        return HttpError::Io(format!("Error connecting to host `{host}`.").into());
    }
    HttpError::Io(err.to_string().into())
}

type LazyNativeTransport = LazyHttpTransport<
    NativeHttpTransport,
    Box<dyn Fn() -> Result<NativeHttpTransport, HttpError> + Send + Sync>,
>;

/// Unified, lazily-initialized native HTTP client combining `CaCertSource`,
/// `LazyHttpTransport`, `NativeHttpTransport`, and `RewritingHttpClient`.
///
/// Shared by `http:`/`https:` modules, `http:`/`https:` resources, and (in Wave 2)
/// Track C `PackageHost` package downloads.
pub struct NativeHttpClient {
    inner: RewritingHttpClient<LazyNativeTransport>,
    cancel_flag: Arc<CancelFlag>,
}

impl NativeHttpClient {
    pub fn new(
        config: HttpClientConfig,
        ca_certs: CaCertSource,
        cancel_flag: Arc<CancelFlag>,
    ) -> Self {
        let cancel_for_factory = cancel_flag.clone();
        let factory: Box<dyn Fn() -> Result<NativeHttpTransport, HttpError> + Send + Sync> =
            Box::new(move || {
                let tls = ca_certs.build_tls_config()?;
                Ok(NativeHttpTransport::new(tls, cancel_for_factory.clone()))
            });
        let lazy = LazyHttpTransport::new(factory);
        Self {
            inner: RewritingHttpClient::new(config, lazy),
            cancel_flag,
        }
    }

    pub fn config(&self) -> &HttpClientConfig {
        self.inner.config()
    }

    pub fn cancel(&self) {
        self.cancel_flag.cancel();
    }

    pub fn close(&self) {
        self.cancel_flag.cancel();
        self.inner.close();
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    pub fn get<F>(&self, original_uri: &str, checker: F) -> Result<HttpResponse, HttpError>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        self.inner.get(original_uri, checker)
    }

    pub fn get_with_uri<F>(
        &self,
        original_uri: &str,
        checker: F,
    ) -> Result<(HttpResponse, String), HttpError>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        self.inner.get_with_uri(original_uri, checker)
    }

    pub fn fetch_module<F>(
        &self,
        original_uri: &str,
        checker: F,
    ) -> Result<Vec<u8>, pkl_runtime::modules::ModuleHostError>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        self.inner.fetch_module(original_uri, checker)
    }

    pub fn fetch_resource<F>(
        &self,
        original_uri: &str,
        checker: F,
    ) -> Result<Option<Vec<u8>>, pkl_runtime::host::ReadFailure>
    where
        F: FnMut(&str) -> Result<(), PklString>,
    {
        self.inner.fetch_resource(original_uri, checker)
    }
}

impl Drop for NativeHttpClient {
    fn drop(&mut self) {
        self.close();
    }
}
