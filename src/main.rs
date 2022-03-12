use clap::{Arg, Command};

mod client;
mod comm;
mod server;

fn main() {
    let res = Command::new("werewolf")
        .arg(
            Arg::new("host")
                .required_unless_present("ip")
                .long("host")
                .help("Hosts a game"),
        )
        .arg(
            Arg::new("ip")
                .required_unless_present("host")
                .takes_value(true)
                .long("ip")
                .help("IP address of the game to connect to (if not hosting)"),
        )
        .arg(
            Arg::new("port")
                .takes_value(true)
                .default_value("57079")
                .long("port")
                .short('p')
                .help("Port to host on or connect to (optional)"),
        )
        .get_matches();

    let port: u16 = res.value_of_t_or_exit("port");

    let game_address = if res.is_present("host") {
        // Hosting the game, so start a server.
        server::start(port)
    } else {
        std::net::SocketAddr::new(res.value_of_t_or_exit("ip"), port)
    };

    // Even if we're hosting the game, we need to connect to the server.
    client::start(game_address);
}
