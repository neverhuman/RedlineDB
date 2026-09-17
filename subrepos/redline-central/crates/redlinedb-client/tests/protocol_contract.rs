use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use redlinedb_client::{Client, Error, Value};

#[test]
fn release_identity_is_native_redline_4_1_0() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "4.1.0");
}

#[test]
fn wire_values_round_trip_without_type_loss() {
    let values = vec![
        Value::Null,
        Value::Integer(-7),
        Value::Real(3.5),
        Value::Text("central".to_owned()),
        Value::Blob(vec![0, 1, 255]),
    ];
    let encoded = serde_json::to_vec(&values).expect("serialize values");
    let decoded: Vec<Value> = serde_json::from_slice(&encoded).expect("deserialize values");
    assert_eq!(decoded, values);
}

#[test]
fn connect_rejects_invalid_server_magic() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("test address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        stream.write_all(b"NOTRLDB!").expect("write invalid magic");
    });

    let error = match Client::connect(&address.to_string()) {
        Ok(_) => panic!("invalid magic must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Protocol(message) if message == "server magic mismatch"));
    server.join().expect("test server completed");
}

#[test]
fn connect_accepts_matching_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("test address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        stream
            .write_all(&[b'R', b'L', b'D', b'B', 0, 1, 0, 0])
            .expect("write server magic");
        let mut client_magic = [0_u8; 8];
        stream
            .read_exact(&mut client_magic)
            .expect("read client magic");
        assert_eq!(client_magic, [b'R', b'L', b'D', b'B', 0, 1, 0, 0]);

        let mut length = [0_u8; 4];
        stream.read_exact(&mut length).expect("read request length");
        let mut request = vec![0_u8; u32::from_be_bytes(length) as usize];
        stream.read_exact(&mut request).expect("read hello request");
        assert_eq!(request, br#"{"cmd":"hello"}"#);

        let response = br#"{"status":"hello","protocol_version":1,"server":"test"}"#;
        stream
            .write_all(&(response.len() as u32).to_be_bytes())
            .expect("write response length");
        stream.write_all(response).expect("write hello response");
    });

    let client = Client::connect(&address.to_string()).expect("matching handshake succeeds");
    drop(client);
    server.join().expect("test server completed");
}
