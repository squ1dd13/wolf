use std::{
    io::{Read, Write},
    net::SocketAddr,
};

pub fn start(port: u16) -> SocketAddr {
    let addr = SocketAddr::new(local_ip_address::local_ip().unwrap(), port);

    println!("Hosting on {}", addr);

    std::thread::spawn(move || run_server(addr));
    addr
}

fn run_server(addr: SocketAddr) {
    let listener = std::net::TcpListener::bind(addr).expect("Unable to start server");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connected to: {}", stream.peer_addr().unwrap());
                std::thread::spawn(move || handle_stream(stream));
            }
            Err(err) => {
                eprintln!("Failed to connect to incoming stream: {}", err);
                continue;
            }
        }
    }
}

fn handle_stream(mut stream: std::net::TcpStream) {
    let mut buf = [0u8; 512];

    loop {
        let message: crate::comm::CtsMessage = match bincode::deserialize_from(&mut stream) {
            Ok(v) => v,
            Err(err) => {
                println!(
                    "Failed to read CTS message from {}: {}; disconnecting",
                    stream.peer_addr().unwrap(),
                    err
                );

                return;
            }
        };

        println!("Got {:?} from {}", message, stream.peer_addr().unwrap());

        let response_bytes = bincode::serialize(&crate::comm::StcMessage::WolvesAwake).unwrap();
        let _ = stream.write(&response_bytes[..]).unwrap();
    }
}
