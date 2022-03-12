use std::{
    collections::HashMap,
    net::{SocketAddr, TcpStream},
    ops::DerefMut,
};

use parking_lot::Mutex;
use rand::Rng;

use crate::comm::{CtsMessage, Role, StcMessage, Winner};

pub fn start(port: u16) -> SocketAddr {
    let addr = SocketAddr::new(local_ip_address::local_ip().unwrap(), port);

    println!("Hosting on {}", addr);

    std::thread::spawn(move || run_server(addr));
    addr
}

fn run_server(addr: SocketAddr) {
    let listener = std::net::TcpListener::bind(addr).expect("Unable to start server");

    let mut game = Game { players: vec![] };

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Add a new player for the stream.
                game.add_player(Player::new(stream));
            }
            Err(err) => {
                eprintln!("Failed to connect to incoming stream: {}", err);
                continue;
            }
        }
    }

    game.play();
}

/// A player in the game.
struct Player {
    /// The stream through which we communicate with the client.
    stream: Mutex<TcpStream>,

    /// Whether the player has died (either by being killed or voted out).
    dead: bool,

    /// The name chosen by the player. This should be unique within the game, as it will be used to
    /// identify individual players to each other.
    name: String,

    /// The player's role.
    role: Option<Role>,
}

impl Player {
    fn new(mut stream: TcpStream) -> Player {
        // We need a message to specify the player's name.
        let msg: CtsMessage = bincode::deserialize_from(&mut stream).unwrap();

        let name = match msg {
            CtsMessage::Name(name) => name,
            msg => panic!("Expected name message, got {:?} instead", msg),
        };

        Player {
            stream: Mutex::new(stream),
            dead: false,
            name,
            role: None,
        }
    }

    /// Sends a message to the client.
    fn send(&self, msg: &StcMessage) {
        let mut stream = self.stream.lock();
        bincode::serialize_into(stream.deref_mut(), &msg).unwrap();
    }

    /// Receives a message from the client.
    fn receive(&self) -> CtsMessage {
        let mut stream = self.stream.lock();
        bincode::deserialize_from(stream.deref_mut()).unwrap()
    }

    /// Returns the player's role. Panics if the role has not been assigned yet.
    fn role(&self) -> Role {
        self.role.unwrap()
    }
}

struct Game {
    /// The players participating in the game.
    players: Vec<Player>,
}

impl Game {
    fn play(&mut self) {
        self.assign_roles();

        loop {
            let killed_name = self.play_night();

            // Play one day, and if either side wins, report that and end the game.
            if let Some(winning_side) = self.play_day(killed_name) {
                self.send_all(&StcMessage::AnnounceWinner(winning_side));
                break;
            }
        }
    }

    /// Adds the given player to the game after announcing that they are joining.
    fn add_player(&mut self, player: Player) {
        self.send_all(&StcMessage::AnnounceJoin(player.name.clone()));
        self.players.push(player);
    }

    /// Assigns a random role to each player.
    fn assign_roles(&mut self) {
        let mut rng = rand::thread_rng();

        // Pick a wolf. Once we've done that, we know the rest of the players are villagers.
        // This will have to change when we add support for multiple wolves, but for now this is
        // fine.
        let wolf_index = rng.gen_range(0..self.players.len());

        for i in 0..self.players.len() {
            let role = if i == wolf_index {
                Role::Wolf
            } else {
                Role::Villager
            };

            self.players[wolf_index].role = Some(role);
            self.players[wolf_index].send(&StcMessage::RoleAssigned(role));
        }
    }

    /// Plays through one night in the game, returning the name of the player killed by the
    /// werewolf.
    fn play_night(&mut self) -> String {
        // Tell all the players that night has fallen.
        self.send_all(&StcMessage::NightFalls);

        // Tell all the players that the wolves have woken up.
        self.send_all(&StcMessage::WolvesWake);

        // Find the wolf in the players so we can ask them who to kill.
        let wolf = self
            .players
            .iter()
            .find(|p| matches!(p.role(), Role::Wolf))
            .unwrap();

        // Find the non-wolf players. These are the players that can be killed by the wolf.
        let kill_names: Vec<String> = self
            .players
            .iter()
            .filter_map(|p| match p.role() {
                Role::Wolf => None,
                _ if !p.dead => Some(p.name.clone()),
                _ => None,
            })
            .collect();

        // Send the wolf the list of players that they can kill. This should trigger their client
        // to ask them for and send back their choice of player.
        wolf.send(&StcMessage::KillOptions(kill_names.clone()));

        // Wait for the player's client to send back their choice of victim.
        let kill_num = match wolf.receive() {
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
            .filter(|p| !matches!(p.role(), Role::Wolf))
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
        self.send_all(&StcMessage::Died(killed_name));

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
            player.send(&StcMessage::VoteOptions(vote_names.clone()));
        }

        // We don't want to allow a player to vote multiple times, so store votes in a hashmap to
        // ensure that there is only one vote per player ID.
        let mut votes = HashMap::<String, usize>::new();

        for player in &living {
            // Say who we're waiting for so players can tell others that they need to vote.
            self.send_all(&StcMessage::WaitingFor(player.name.clone()));

            match player.receive() {
                CtsMessage::Vote(vote) => {
                    // Tell all the players about the vote.
                    self.send_all(&StcMessage::AnnounceVote(
                        player.name.clone(),
                        living[vote].name.clone(),
                    ));

                    // Record the vote.
                    votes.insert(player.name.clone(), vote);
                }

                msg => {
                    println!("Expected vote message, got {:?} instead", msg);
                }
            };
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
            self.send_all(&StcMessage::VotedOut(living[voted_index].name.clone()));

            // Drop the living players vector so we can get a mutable reference to the player and
            // kill them. (We need to drop the immutable references first, or we'd be mutably
            // borrowing the players when there are still immutable references around.)
            drop(living);

            let mut living_mut: Vec<_> = self.players.iter_mut().filter(|p| !p.dead).collect();

            // Get a mutable reference to the player who has been voted out.
            let voted: &mut _ = living_mut[voted_index];

            // Kill them.
            voted.dead = true;

            if let Role::Wolf = voted.role() {
                // The village wins, because the wolf was killed.
                return Some(Winner::Village);
            } else if living_mut.len() == 2 {
                // The wolves win, because the number of wolves left is equal to the number of
                // villagers left (one wolf and one villager).
                return Some(Winner::Wolf);
            }

            // Nobody wins yet, so the game will continue into another night.
        } else {
            self.send_all(&StcMessage::NoMajority);
        }

        None
    }

    /// Sends the given message to every player.
    fn send_all(&self, message: &StcMessage) {
        for player in &self.players {
            player.send(message);
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
