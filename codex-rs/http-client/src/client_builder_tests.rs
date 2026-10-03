use super::*;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn default_tls_negotiates_http2() {
    ensure_rustls_crypto_provider();
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .expect("self-signed certificate should generate");
    let trusted_certificate = reqwest::Certificate::from_der(certificate.cert.der())
        .expect("server certificate should parse");
    let private_key = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der()),
    );
    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![certificate.cert.der().clone()], private_key)
        .expect("TLS server should be configured");
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("TLS server should bind");
    let address = listener
        .local_addr()
        .expect("TLS server should have an address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("TLS server should accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("TLS handshake should have a timeout");
        let mut connection = rustls::ServerConnection::new(Arc::new(config))
            .expect("TLS server connection should be created");
        connection
            .complete_io(&mut stream)
            .expect("TLS handshake should succeed");
        connection.alpn_protocol().map(<[u8]>::to_vec)
    });
    let client = HttpClientBuilder::new()
        .reqwest_builder(ProxyRouting::Direct)
        .add_root_certificate(trusted_certificate)
        .timeout(Duration::from_secs(5))
        .build()
        .expect("default TLS client should build");

    // The fixture closes after the handshake; only protocol negotiation is under test.
    let _ = client
        .get(format!("https://localhost:{}/", address.port()))
        .send()
        .await;
    assert_eq!(
        server.join().expect("TLS server should finish"),
        Some(b"h2".to_vec())
    );
}

#[tokio::test]
async fn custom_ca_fallback_preserves_builder_configuration() {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("HTTP listener should bind");
    let address = listener
        .local_addr()
        .expect("HTTP listener should have an address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("HTTP listener should accept");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let bytes_read = stream.read(&mut chunk).expect("HTTP request should read");
            assert!(bytes_read > 0, "HTTP request should include headers");
            request.extend_from_slice(&chunk[..bytes_read]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("HTTP listener should write response");
        String::from_utf8(request).expect("HTTP request should be UTF-8")
    });
    let mut headers = HeaderMap::new();
    headers.insert("x-builder-test", HeaderValue::from_static("preserved"));
    let client = HttpClientBuilder::new()
        .default_headers(headers)
        .build_with_custom_ca_fallback_using(ProxyRouting::Direct, |_| {
            Err(BuildCustomCaTransportError::InvalidCaFile {
                source_env: "TEST_CA_ENV",
                path: PathBuf::from("invalid-test-ca.pem"),
                detail: "synthetic invalid CA".to_string(),
            })
        });

    let response = client
        .get(format!("http://{address}/fallback"))
        .send()
        .await
        .expect("fallback client should send request");
    assert!(response.status().is_success());
    let request = server.join().expect("HTTP listener should finish");
    assert!(
        request
            .lines()
            .any(|line| line.eq_ignore_ascii_case("x-builder-test: preserved"))
    );
}
