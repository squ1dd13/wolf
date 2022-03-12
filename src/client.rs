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

    /// Sender for sending messages to the server.
    sender: crossbeam::channel::Sender<CtsMessage>,

    /// Receiver for receiving messages from the server.
    receiver: crossbeam::channel::Receiver<StcMessage>,
}

impl Player {
    fn play(&mut self) {
        for msg in self.receiver.try_iter() {
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
                    self.sender.send(CtsMessage::Vote(vote)).unwrap();
                }
                StcMessage::KillOptions(opts) => {
                    let kill = self.ask_kill(opts);
                    self.sender.send(CtsMessage::Kill(kill)).unwrap();
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
    }

    fn ask_vote(&self, opts: Vec<String>) -> usize {
        todo!()
    }

    fn ask_kill(&self, opts: Vec<String>) -> usize {
        todo!()
    }
}
