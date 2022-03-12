use std::{
    collections::HashMap,
    io::{Read, Write},
    net::SocketAddr,
};

use crate::comm::{CtsMessage, Role, StcMessage, Winner};

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

        let response_bytes = bincode::serialize(&crate::comm::StcMessage::WolvesWake).unwrap();
        let _ = stream.write(&response_bytes[..]).unwrap();
    }
}

/// A player in the game.
struct Player {
    /// Sender for sending messages to the player's device over the network.
    sender: crossbeam::channel::Sender<crate::comm::StcMessage>,

    /// Receiver for receiving messages from the player's computer.
    receiver: crossbeam::channel::Receiver<crate::comm::CtsMessage>,

    /// Whether the player has died (either by being killed or voted out).
    dead: bool,

    /// The name chosen by the player. This should be unique within the game, as it will be used to
    /// identify individual players to each other.
    name: String,

    /// The player's role.
    role: Role,
}

struct Game {
    /// The players participating in the game.
    players: Vec<Player>,
}

impl Game {
    fn play(&mut self) {
        loop {
            let killed_name = self.play_night();
            self.play_day(killed_name);
        }
    }

    /// Plays through one night in the game, returning the name of the player killed by the
    /// werewolf.
    fn play_night(&mut self) -> String {
        // Tell all the players that night has fallen.
        self.send_all(StcMessage::NightFalls);

        // Tell all the players that the wolves have woken up.
        self.send_all(StcMessage::WolvesWake);

        // Find the wolf in the players so we can ask them who to kill.
        let wolf = self
            .players
            .iter()
            .find(|p| matches!(p.role, Role::Wolf))
            .unwrap();

        // Find the non-wolf players. These are the players that can be killed by the wolf.
        let kill_names: Vec<String> = self
            .players
            .iter()
            .filter_map(|p| match p.role {
                Role::Wolf => None,
                _ if !p.dead => Some(p.name.clone()),
                _ => None,
            })
            .collect();

        // Send the wolf the list of players that they can kill. This should trigger their client
        // to ask them for and send back their choice of player.
        wolf.sender
            .send(StcMessage::KillOptions(kill_names.clone()))
            .unwrap();

        // Wait for the player's client to send back their choice of victim.
        let kill_num = match wolf.receiver.recv().unwrap() {
            CtsMessage::Kill(num) => num,
            msg => {
                // We shouldn't get anything else here, so panic if we do.
                panic!("Expected kill message from wolf, but got {:?} instead", msg);
            }
        };

        // Get a reference to the player that the wolf is killing.
        let player_killed = self
            .players
            .iter_mut()
            .filter(|p| !matches!(p.role, Role::Wolf))
            .nth(kill_num)
            .unwrap();

        if player_killed.name != kill_names[kill_num] {
            panic!(
                "Player name mismatch: {} killed, but index gives {} from original vec",
                player_killed.name, kill_names[kill_num]
            );
        }

        // Kill them.
        player_killed.dead = true;

        // Return the name of the killed player for use in the day phase.
        player_killed.name.clone()
    }

    /// Plays through one day in the game, given the name of the player that was killed the night
    /// before.
    ///
    /// If this day ends the game, the winning side will be returned. Otherwise, `None` will be
    /// returned.
    fn play_day(&mut self, killed_name: String) -> Option<Winner> {
        // Tell all the players who died.
        self.send_all(StcMessage::Died(killed_name));

        // Find all the living players. These are the players who will get a vote, and who can be
        // voted against by other players.
        let living = self.players.iter().filter(|p| !p.dead);

        // Get the names of all the players that can be voted against.
        let vote_names: Vec<_> = living.clone().map(|p| p.name.clone()).collect();

        // Create a vector from the iterator of living players so we don't need to keep cloning the
        // iterator.
        let living: Vec<&Player> = living.collect();

        // Send the names to every living player to allow their client to let them vote.
        for player in &living {
            player
                .sender
                .send(StcMessage::VoteOptions(vote_names.clone()))
                .unwrap();
        }

        // We don't want to allow a player to vote multiple times, so store votes in a hashmap to
        // ensure that there is only one vote per player ID.
        let mut votes = HashMap::<String, usize>::new();

        // Wait for votes until every player has voted.
        while votes.len() < living.len() {
            for player in &living {
                match player.receiver.try_recv() {
                    // We should get a vote message from every player.
                    Ok(CtsMessage::Vote(vote)) => {
                        // Tell all the players about the vote.
                        self.send_all(StcMessage::AnnounceVote(
                            player.name.clone(),
                            living[vote].name.clone(),
                        ));

                        // Record the vote.
                        votes.insert(player.name.clone(), vote);
                    }

                    // We should only be receiving votes from clients at this point.
                    Ok(msg) => println!("Expected vote message, got {:?} instead", msg),

                    // Carry on waiting if there is nothing there.
                    Err(crossbeam::channel::TryRecvError::Empty) => continue,

                    // Other errors are "real" errors.
                    Err(err) => panic!("try_recv error: {}", err),
                }
            }
        }

        // Count the votes by making a new hashmap with the player index as a key, and the number
        // of votes they have received as the value.
        let mut vote_counts = HashMap::<usize, usize>::new();

        for (_, player_index) in votes {
            *vote_counts.entry(player_index).or_default() += 1;
        }

        // Find the player with the most votes.
        let (&voted_index, &num_votes) = vote_counts.iter().max_by_key(|(_, &num)| num).unwrap();

        // Check if the vote has a majority (i.e. whether more than half of the players agreed).
        if num_votes > (living.len() / 2) {
            // Majority vote, so the person should die.
            self.send_all(StcMessage::VotedOut(living[voted_index].name.clone()));

            // Drop the living players vector so we can get a mutable reference to the player and
            // kill them. (We need to drop the immutable references first, or we'd be mutably
            // borrowing the players when there are still immutable references around.)
            drop(living);

            let mut living_mut: Vec<_> = self.players.iter_mut().filter(|p| !p.dead).collect();

            // Get a mutable reference to the player who has been voted out.
            let voted: &mut _ = living_mut[voted_index];

            // Kill them.
            voted.dead = true;

            if let Role::Wolf = voted.role {
                // The village wins, because the wolf was killed.
                return Some(Winner::Village);
            } else if living_mut.len() == 2 {
                // The wolves win, because the number of wolves left is equal to the number of
                // villagers left (one wolf and one villager).
                return Some(Winner::Wolf);
            }

            // Nobody wins yet, so the game will continue into another night.
        } else {
            self.send_all(StcMessage::NoMajority);
        }

        None
    }

    /// Sends the given message to every player.
    fn send_all(&self, message: StcMessage) {
        for player in &self.players {
            player.sender.send(message.clone()).unwrap();
        }
    }
}

/*

    Host starts program
    Other players connect, giving a name
    Host starts game
    Players allocated roles

    Night falls
    Werewolf asked who they wish to kill
    Werewolf enters a number from a list of valid targets
    Game kills specified target

    Daytime
    Players told who died
    (Maybe add chat functionality, but not now as VC/IRL is much better)
    Each living player allowed to vote for any living player (including themselves)
    Everyone can see who votes for who
    If werewolf voted out, game ends
    Otherwise, back to night

*/
