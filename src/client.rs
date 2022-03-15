use std::{collections::HashMap, io::Write, net::SocketAddr};

use crate::comm::{CtsMessage, PlayerId, Role, StcMessage, Winner};
use parking_lot::Mutex;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub fn start(addr: SocketAddr) {
    println!("Connecting to {}", addr);
    Player::new(Session::new(addr)).play();
}

/// A coloured output stream that abstracts semantic highlighting details.
struct Output {
    stdout: Mutex<StandardStream>,
}

impl Output {
    /// Creates a new coloured stdout stream.
    fn new() -> Output {
        Output {
            stdout: Mutex::new(StandardStream::stdout(ColorChoice::Always)),
        }
    }

    /// Writes the given string to the stream.
    fn write(&self, name: impl AsRef<str>) {
        write!(self.stdout.lock(), "{}", name.as_ref()).unwrap();
    }

    /// Sets the stream's foreground colour and whether the text is bold.
    fn set_fg(&self, colour: Color, bold: bool) {
        self.stdout
            .lock()
            .set_color(ColorSpec::new().set_fg(Some(colour)).set_bold(bold))
            .unwrap();
    }

    /// Resets the stream's styling options.
    fn reset(&self) {
        self.stdout.lock().reset().unwrap();
    }

    /// Writes a player name to the stream.
    fn write_name(&self, name: impl AsRef<str>) {
        self.set_fg(Color::Blue, true);
        self.write(name);
        self.reset();
    }

    /// Writes information important to the user to the stream.
    fn write_user(&self, msg: impl AsRef<str>) {
        self.set_fg(Color::Green, false);
        self.write(msg);
        self.reset();
    }

    /// Writes general game information to the stream.
    fn write_log(&self, msg: impl AsRef<str>) {
        self.write(msg);
    }
}

/// A connection to a game room.
struct Session {
    /// The stream used to talk to room on the server.
    stream: std::net::TcpStream,

    /// The names of the players in the session.
    players: HashMap<PlayerId, String>,
}

impl Session {
    /// Creates a new `Session` by connecting to the given address over TCP.
    fn new(addr: SocketAddr) -> Session {
        Session {
            stream: std::net::TcpStream::connect(addr).unwrap(),
            players: HashMap::new(),
        }
    }

    fn send(&mut self, msg: CtsMessage) {
        bincode::serialize_into(&mut self.stream, &msg).unwrap();
    }

    fn receive(&mut self) -> StcMessage {
        bincode::deserialize_from(&mut self.stream).unwrap()
    }
}

/// The user's player. Manages communication with the host.
struct Player {
    /// The ID of this player.
    id: PlayerId,

    /// The name chosen by the user.
    name: String,

    /// The coloured output stream.
    output: Output,

    /// The player's role.
    role: Option<Role>,

    /// Whether the player has died.
    dead: bool,

    /// The session that the player is currently in.
    session: Session,
}

impl Player {
    /// Creates a new player connected to the given session.
    fn new(mut session: Session) -> Player {
        // Ask the user for a name to connect with.
        let name = Self::input_name();

        // Ask to connect to the session with the name the user entered.
        session.send(CtsMessage::Connect(name.clone()));

        // The server should register the player with an ID and send it back so we can identify
        // ourselves by ID. (The server uses the ID to identify players in messages, so we need to
        // have one as soon as we connect.)
        let id = match session.receive() {
            StcMessage::IdAssigned(id) => id,
            msg => panic!("Expected to receive player ID, but got {:?} instead", msg),
        };

        // Acknowledge receipt of the ID.
        session.send(CtsMessage::Received);

        Player {
            id,
            name,
            output: Output::new(),

            // No role yet, since the server can only pick roles once all the players have
            // joined and the game is about to start.
            role: None,

            dead: false,
            session,
        }
    }

    /// Enters a loop of waiting for messages from the host and responding to them.
    fn play(&mut self) {
        loop {
            let msg = bincode::deserialize_from(&mut self.session.stream).unwrap();

            if let Some(winner) = self.handle_message(msg) {
                match winner {
                    Winner::Wolf => self.output.write_user(
                        r#"The werewolves win.
The number of villagers remaining is equal to the number of werewolves."#,
                    ),
                    Winner::Village => self.output.write_user(
                        r#"The villagers win.
All of the werewolves have been killed."#,
                    ),
                }

                break;
            }
        }
    }

    /// Does something with the given message from the host.
    fn handle_message(&mut self, msg: StcMessage) -> Option<Winner> {
        match msg {
            StcMessage::WolvesWake => {
                self.output.write_log("The wolves wake.\n");
                self.send_ack();
            }

            StcMessage::NightFalls => {
                self.output.write_log("Night has fallen.\n");
                self.send_ack();
            }

            StcMessage::Died(name) => {
                if name == self.name {
                    self.output.write_user("You were killed last night.\n");
                    self.dead = true;
                } else {
                    self.output.write_name(name);
                    self.output.write_log(" was killed last night.\n");
                }

                self.send_ack();
            }

            StcMessage::VoteOptions(opts) => {
                let vote = self.ask_vote(opts);
                self.send(CtsMessage::Vote(vote));
            }

            StcMessage::KillOptions(opts) => {
                let kill = self.ask_kill(opts);
                self.send(CtsMessage::Kill(kill));
            }

            StcMessage::AnnounceVote(name, against) => {
                self.output.write_name(name);
                self.output.write_log(" voted against ");
                self.output.write_name(against);
                self.output.write_log(".\n");

                self.send_ack();
            }

            StcMessage::NoMajority => {
                self.output.write_log("There was no majority vote.\n");
                self.send_ack();
            }

            StcMessage::VotedOut(name) => {
                if name == self.name {
                    self.output
                        .write_user("You were voted out by the other players.\n");
                    self.dead = true;
                } else {
                    self.output.write_name(name);
                    self.output
                        .write_log(" was voted out by the other players.\n");
                }

                self.send_ack();
            }

            StcMessage::RoleAssigned(role) => {
                self.role = Some(role);

                // Tell the player what their role is, and what they are supposed to do.
                let (role_name, desc) = match role {
                    Role::Wolf => ("werewolf", "Kill others and avoid detection."),
                    Role::Villager => (
                        "villager",
                        "Do villager things, avoid being killed, and capture the werewolves.",
                    ),
                };

                self.output
                    .write_user(format!("Your role is {}.\n", role_name));
                self.output.write(desc);
                println!();

                self.send_ack();
            }

            StcMessage::AnnounceWinner(winner) => return Some(winner),

            StcMessage::WaitingFor(name) => {
                if name == self.name {
                    self.output.write_user("It's your turn to vote.\n");
                } else {
                    self.output.write_log("Waiting for ");
                    self.output.write_name(name);
                    self.output.write_log(" to vote.\n");
                }

                self.send_ack();
            }

            StcMessage::AnnounceJoin(id, name) => {
                self.output.write_name(&name);
                self.output.write_log(" joined the game.\n");
                self.send_ack();

                self.session.players.insert(id, name);
            }

            StcMessage::Players(map) => {
                self.session.players.extend(map.into_iter());
                self.send_ack();
            }

            msg => println!("Unhandled message {:?} in loop", msg),
        }

        None
    }

    /// Sends the `Received` message to the host.
    fn send_ack(&mut self) {
        self.send(CtsMessage::Received);
    }

    /// Sends the given message to the host.
    fn send(&mut self, msg: CtsMessage) {
        bincode::serialize_into(&mut self.session.stream, &msg).unwrap();
    }

    fn show_menu(
        &self,
        title: impl AsRef<str>,
        prompt: impl AsRef<str>,
        opts: Vec<String>,
    ) -> usize {
        let mut line = String::new();

        loop {
            self.output.write_user(title.as_ref());

            for (i, name) in opts.iter().enumerate() {
                self.output.write(format!("  [{}] {}", i + 1, name));
            }

            println!();
            self.output
                .write_user(format!("{} (1 to {}): ", prompt.as_ref(), opts.len()));
            std::io::stdout().flush().unwrap();

            std::io::stdin().read_line(&mut line).unwrap();

            if let Ok(num) = line.trim().parse::<usize>() {
                if (1..=opts.len()).contains(&num) {
                    // Subtract one to turn the number into an index again.
                    break num - 1;
                }
            }

            self.output.write("Invalid input. Please try again.\n");
            line.clear();
        }
    }

    /// Presents the user with a voting menu, given a vector of names of players that could be
    /// voted against.
    ///
    /// Returns the index of the person the player votes against.
    fn ask_vote(&self, opts: Vec<String>) -> usize {
        self.show_menu("Who do you want to vote out?", "Your vote", opts)
    }

    /// Presents the user with a kill menu, given a vector of names of potential victims.
    ///
    /// Returns the index of the person the player chooses to kill.
    fn ask_kill(&self, opts: Vec<String>) -> usize {
        self.show_menu("Who do you want to kill?", "Your victim", opts)
    }

    /// Gets a valid player name from the user.
    fn input_name() -> String {
        let mut name = String::new();

        loop {
            print!("Please enter your name: ");
            std::io::stdout().flush().unwrap();
            std::io::stdin().read_line(&mut name).unwrap();

            let trimmed = name.trim();

            if trimmed.is_empty() {
                println!("You can't have an empty name! Try again.");

                name.clear();
                continue;
            }

            break trimmed.to_string();
        }
    }
}
