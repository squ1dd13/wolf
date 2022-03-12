use std::{io::Write, net::SocketAddr};

use crate::comm::{CtsMessage, Role, StcMessage};

pub fn start(addr: SocketAddr) {
    let mut stream = std::net::TcpStream::connect(addr).unwrap();

    let msg = bincode::serialize(&crate::comm::CtsMessage::Text("Hello".to_string())).unwrap();
    stream.write_all(&msg[..]).unwrap();

    let resp: crate::comm::StcMessage = bincode::deserialize_from(&mut stream).unwrap();
    println!("Got response: {:?}", resp);
}

/// The user's player. Manages communication with the host.
struct Player {
    /// Whether the player has died.
    dead: bool,

    /// The name chosen by the user.
    name: String,

    /// The player's role.
    role: Role,

    /// The stream through which we communicate with the host.
    stream: std::net::TcpStream,
}

impl Player {
    /// Enters a loop of waiting for messages from the host and responding to them.
    fn play(&mut self) {
        loop {
            let msg = bincode::deserialize_from(&mut self.stream).unwrap();
            self.handle_message(msg);
        }
    }

    /// Does something with the given message from the host.
    ///
    /// This may or may not involve sending a message back to the host - in a lot of cases, there
    /// is no need to respond.
    fn handle_message(&mut self, msg: StcMessage) {
        match msg {
            StcMessage::WolvesWake => {
                println!("The wolves wake.");
            }

            StcMessage::NightFalls => {
                println!("Night has fallen.");
            }

            StcMessage::Died(name) => {
                if name == self.name {
                    println!("You were killed last night.");
                    self.dead = true;
                } else {
                    println!("{} was killed last night.", name);
                }
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
            }

            StcMessage::NoMajority => {
                println!("There was no majority vote.");
            }

            StcMessage::VotedOut(name) => {
                if name == self.name {
                    println!("You were voted out by the other players.");
                    self.dead = true;
                } else {
                    println!("{} was voted out by the other players.", name);
                }
            }
        }
    }

    /// Sends the given message to the host.
    fn send(&mut self, msg: CtsMessage) {
        bincode::serialize_into(&mut self.stream, &msg).unwrap();
    }

    /// Presents the user with a voting menu, given a vector of names of players that could be
    /// voted against.
    ///
    /// Returns the index of the person the player votes against.
    fn ask_vote(&self, opts: Vec<String>) -> usize {
        todo!()
    }

    /// Presents the user with a kill menu, given a vector of names of potential victims.
    ///
    /// Returns the index of the person the player chooses to kill.
    fn ask_kill(&self, opts: Vec<String>) -> usize {
        todo!()
    }
}
