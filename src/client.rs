use std::{io::Write, net::SocketAddr};

use crate::comm::{CtsMessage, Role, StcMessage, Winner};

pub fn start(addr: SocketAddr) {
    let mut player = Player {
        name: Player::input_name(),
        role: None,
        dead: false,
        stream: std::net::TcpStream::connect(addr).unwrap(),
    };

    player.play();
}

/// The user's player. Manages communication with the host.
struct Player {
    /// The name chosen by the user.
    name: String,

    /// The player's role.
    role: Option<Role>,

    /// Whether the player has died.
    dead: bool,

    /// The stream through which we communicate with the host.
    stream: std::net::TcpStream,
}

impl Player {
    /// Enters a loop of waiting for messages from the host and responding to them.
    fn play(&mut self) {
        // The first message must be us sending the player's name across.
        self.send(CtsMessage::Name(self.name.clone()));

        loop {
            let msg = bincode::deserialize_from(&mut self.stream).unwrap();

            if let Some(winner) = self.handle_message(msg) {
                match winner {
                    Winner::Wolf => println!(
                        r#"The werewolves win.
The number of villagers remaining is equal to the number of werewolves."#
                    ),
                    Winner::Village => println!(
                        r#"The villagers win.
All of the werewolves have been killed."#
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
                println!("The wolves wake.");
                self.send_ack();
            }

            StcMessage::NightFalls => {
                println!("Night has fallen.");
                self.send_ack();
            }

            StcMessage::Died(name) => {
                if name == self.name {
                    println!("You were killed last night.");
                    self.dead = true;
                } else {
                    println!("{} was killed last night.", name);
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
                println!("{} voted against {}.", name, against);
                self.send_ack();
            }

            StcMessage::NoMajority => {
                println!("There was no majority vote.");
                self.send_ack();
            }

            StcMessage::VotedOut(name) => {
                if name == self.name {
                    println!("You were voted out by the other players.");
                    self.dead = true;
                } else {
                    println!("{} was voted out by the other players.", name);
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

                println!("Your role is {}. {}", role_name, desc);
                self.send_ack();
            }

            StcMessage::AnnounceWinner(winner) => return Some(winner),

            StcMessage::WaitingFor(name) => {
                if name == self.name {
                    println!("It's your turn to vote.");
                } else {
                    println!("Waiting for {} to vote.", name);
                }

                self.send_ack();
            }

            StcMessage::AnnounceJoin(name) => {
                println!("{} joined the game.", name);
                self.send_ack();
            }
        }

        None
    }

    /// Sends the `Received` message to the host.
    fn send_ack(&mut self) {
        self.send(CtsMessage::Received);
    }

    /// Sends the given message to the host.
    fn send(&mut self, msg: CtsMessage) {
        bincode::serialize_into(&mut self.stream, &msg).unwrap();
    }

    fn show_menu(title: impl AsRef<str>, prompt: impl AsRef<str>, opts: Vec<String>) -> usize {
        let mut line = String::new();

        loop {
            println!("{}", title.as_ref());

            for (i, name) in opts.iter().enumerate() {
                println!("  [{}] {}", i + 1, name);
            }

            println!();
            print!("{} (1 to {}): ", prompt.as_ref(), opts.len());
            std::io::stdout().flush().unwrap();

            std::io::stdin().read_line(&mut line).unwrap();

            if let Ok(num) = line.trim().parse::<usize>() {
                if (1..=opts.len()).contains(&num) {
                    // Subtract one to turn the number into an index again.
                    break num - 1;
                }
            }

            println!("Invalid input. Please try again.");
            line.clear();
        }
    }

    /// Presents the user with a voting menu, given a vector of names of players that could be
    /// voted against.
    ///
    /// Returns the index of the person the player votes against.
    fn ask_vote(&self, opts: Vec<String>) -> usize {
        Self::show_menu("Who do you want to vote out?", "Your vote", opts)
    }

    /// Presents the user with a kill menu, given a vector of names of potential victims.
    ///
    /// Returns the index of the person the player chooses to kill.
    fn ask_kill(&self, opts: Vec<String>) -> usize {
        Self::show_menu("Who do you want to kill?", "Your victim", opts)
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
