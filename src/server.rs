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
        match stream.read(&mut buf) {
            Ok(len) => {
                // Read a single CTS (client-to-server) message.
                let message: crate::comm::CtsMessage = bincode::deserialize(&buf[..len]).unwrap();
                println!("Got {:?} from {}", message, stream.peer_addr().unwrap());

                let response_bytes =
                    bincode::serialize(&crate::comm::StcMessage::WolvesAwake).unwrap();

                let _ = stream.write(&response_bytes[..]).unwrap();
            }

            Err(err) => {
                println!(
                    "Error reading from stream for {}: {}; disconnecting",
                    stream.peer_addr().unwrap(),
                    err
                );

                return;
            }
        }
    }
}
