use std::{io::Write, net::SocketAddr};

pub fn start(addr: SocketAddr) {
    let mut stream = std::net::TcpStream::connect(addr).unwrap();

    let msg = bincode::serialize(&crate::comm::CtsMessage::Text("Hello".to_string())).unwrap();
    stream.write_all(&msg[..]).unwrap();

    let resp: crate::comm::StcMessage = bincode::deserialize_from(&mut stream).unwrap();
    println!("Got response: {:?}", resp);

    loop {}
}
