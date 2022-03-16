use std::{
    collections::HashMap,
    io::Write,
    net::{IpAddr, SocketAddr, TcpStream},
    ops::DerefMut,
};

use parking_lot::Mutex;
use rand::Rng;

use crate::comm::{CtsMessage, PlayerId, Role, StcMessage, Winner};

pub fn start(port: u16) -> SocketAddr {
    let addr = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), port);

    // Create the listener on the calling thread so that this function blocks until the server is
    // ready to listen for messages.
    let listener = std::net::TcpListener::bind(addr).expect("Unable to start server");

    println!("Hosting on {}", addr);

    std::thread::spawn(move || run_server(listener));

    addr
}

fn run_server(listener: std::net::TcpListener) {
    let mut game = Game::new();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Add a new player for the stream.
                Player::join(&mut game, stream);
                std::io::stdout().flush().unwrap();

                std::thread::sleep(std::time::Duration::from_millis(500));

                print!("Do you wish to start the game? y/n: ");
                std::io::stdout().flush().unwrap();

                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf).unwrap();

                if buf.starts_with('y') {
                    break;
                }

                println!("Waiting for more players...");
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
    /// The player's ID. This allows us to refer to this player without needing to identify by
    /// name.
    id: PlayerId,

    /// The name chosen by the player. This should be unique within the game, as it will be used to
    /// identify individual players to each other.
    name: String,

    /// The stream through which we communicate with the client.
    stream: Mutex<TcpStream>,

    /// Whether the player has died (either by being killed or voted out).
    dead: bool,

    /// The player's role.
    role: Option<Role>,
}

impl Player {
    /// Creates a new `Player` for the given stream, and add the player to a game.
    fn join(game: &mut Game, mut stream: TcpStream) {
        // We need a message to specify the player's name.
        let msg: CtsMessage = bincode::deserialize_from(&mut stream).unwrap();

        let name = match msg {
            CtsMessage::Connect(name) => name,
            msg => panic!("Expected name message, got {:?} instead", msg),
        };

        // Get the game to generate a new ID for this player.
        let id = game.take_next_id();

        let player = Player {
            id,
            stream: Mutex::new(stream),
            dead: false,
            name,
            role: None,
        };

        // Send the ID to the player's client so that they know what their own ID is.
        player.send(&StcMessage::IdAssigned(id));

        // Create the new player and add them to the game.
        game.add_player(player);
    }

    /// Sends a message to the client.
    fn send(&self, msg: &StcMessage) -> CtsMessage {
        println!("server sending: {:?}", msg);

        let mut stream = self.stream.lock();
        bincode::serialize_into(stream.deref_mut(), &msg).unwrap();

        // Every message sent from the host should prompt a response from the client.
        let resp = bincode::deserialize_from(stream.deref_mut()).unwrap();

        println!("got back: {:?}", resp);
        resp
    }

    /// Returns the player's role. Panics if the role has not been assigned yet.
    fn role(&self) -> Role {
        self.role.expect("No role given")
    }
}

struct Game {
    /// The players participating in the game.
    players: HashMap<PlayerId, Player>,

    /// The next available player ID for this game.
    next_id: PlayerId,
}

impl Game {
    fn new() -> Game {
        Game {
            players: HashMap::new(),
            next_id: PlayerId::new(),
        }
    }

    fn play(&mut self) {
        self.assign_roles();

        loop {
            let killed_id = self.play_night();

            // Play one day, and if either side wins, report that and end the game.
            if let Some(winning_side) = self.play_day(killed_id) {
                self.send_all(&StcMessage::AnnounceWinner(winning_side));
                break;
            }
        }
    }

    /// Adds the given player to the game after announcing that they are joining.
    fn add_player(&mut self, player: Player) {
        self.send_all(&StcMessage::AnnounceJoin(player.id, player.name.clone()));

        // Tell the new player about all the players who are already in the game.
        player.send(&StcMessage::Players(
            self.players
                .iter()
                .map(|(&id, p)| (id, p.name.clone()))
                .collect(),
        ));

        self.players.insert(player.id, player);
    }

    /// Returns a player ID that can be used for a new player joining the game.
    ///
    /// An identical player ID will not be generated by this `Game` instance on any subsequent call
    /// to this method.
    fn take_next_id(&mut self) -> PlayerId {
        let id = self.next_id;

        // Get another player ID for the next call to this method, since we've used this one
        // already.
        self.next_id = id.next();

        id
    }

    /// Assigns a random role to each player.
    fn assign_roles(&mut self) {
        let mut rng = rand::thread_rng();

        // Pick a wolf. Once we've done that, we know the rest of the players are villagers.
        // This will have to change when we add support for multiple wolves, but for now this is
        // fine.
        let wolf_index = rng.gen_range(0..self.players.len());

        for (i, player) in self.players.values_mut().enumerate() {
            let role = if i == wolf_index {
                Role::Wolf
            } else {
                Role::Villager
            };

            player.role = Some(role);
            player.send(&StcMessage::RoleAssigned(role));
        }
    }

    /// Plays through one night in the game, returning the ID of the player killed by the werewolf.
    fn play_night(&mut self) -> PlayerId {
        // Tell all the players that night has fallen.
        self.send_all(&StcMessage::NightFalls);

        // Tell all the players that the wolves have woken up.
        self.send_all(&StcMessage::WolvesWake);

        // Find the wolf in the players so we can ask them who to kill.
        let wolf = self
            .players
            .values()
            .find(|p| matches!(p.role(), Role::Wolf))
            .unwrap();

        // Find the non-wolf players. These are the players that can be killed by the wolf.
        let kill_candidates: Vec<PlayerId> = self
            .players
            .values()
            .filter_map(|p| match p.role() {
                Role::Wolf => None,
                _ if !p.dead => Some(p.id),
                _ => None,
            })
            .collect();

        // Send the wolf the list of players that they can kill. This should trigger their client
        // to ask them for and send back their choice of player.
        let response = wolf.send(&StcMessage::KillOptions(kill_candidates.clone()));

        let kill_id = match response {
            CtsMessage::Kill(id) => id,
            msg => {
                // We shouldn't get anything else here, so panic if we do.
                panic!("Expected kill message from wolf, but got {:?} instead", msg);
            }
        };

        if !kill_candidates.contains(&kill_id) {
            panic!(
                "Wolf attempted to kill non-candidate {}",
                self.players.get(&kill_id).unwrap().name
            );
        }

        // Get a reference to the player the wolf is killing.
        let player_killed = self.players.get_mut(&kill_id).unwrap();

        // Kill them.
        player_killed.dead = true;

        // Return the ID of the killed player for use in the day phase.
        kill_id
    }

    /// Plays through one day in the game, given the name of the player that was killed the night
    /// before.
    ///
    /// If this day ends the game, the winning side will be returned. Otherwise, `None` will be
    /// returned.
    fn play_day(&mut self, killed_id: PlayerId) -> Option<Winner> {
        // Tell all the players which one died.
        self.send_all(&StcMessage::Died(killed_id));

        // Find all the living players. These are the players who will get a vote, and who can be
        // voted against by other players.
        let living = self.players.values().filter(|p| !p.dead);

        // Get the names of all the players that can be voted against.
        let candidates: Vec<_> = living.clone().map(|p| p.id).collect();

        // Create a vector from the iterator of living players so we don't need to keep cloning the
        // iterator.
        let living: Vec<&Player> = living.collect();

        // We don't want to allow a player to vote multiple times, so store votes in a hashmap to
        // ensure that there is only one vote per player ID.
        let mut votes = HashMap::<String, PlayerId>::new();

        for player in &living {
            // Say who we're waiting for so players can tell others that they need to vote.
            self.send_all(&StcMessage::WaitingFor(player.id));

            let response = player.send(&StcMessage::VoteOptions(candidates.clone()));

            match response {
                CtsMessage::Vote(vote) => {
                    // Tell all the players about the vote.
                    self.send_all(&StcMessage::AnnounceVote(player.id, vote));

                    // Record the vote.
                    votes.insert(player.name.clone(), vote);
                }

                msg => {
                    println!("Expected vote message, got {:?} instead", msg);
                }
            };
        }

        // Count the votes by making a new hashmap with the player ID as a key, and the number of
        // votes they have received as the value.
        let mut vote_counts = HashMap::<PlayerId, usize>::new();

        for (_, player_index) in votes {
            *vote_counts.entry(player_index).or_default() += 1;
        }

        // Find the player with the most votes.
        let (&voted_id, &num_votes) = vote_counts.iter().max_by_key(|(_, &num)| num).unwrap();

        // Check if the vote has a majority (i.e. whether more than half of the players agreed).
        if num_votes > (living.len() / 2) {
            // Majority vote, so the person should die.
            self.send_all(&StcMessage::VotedOut(voted_id));

            // Drop the living players vector so we can get a mutable reference to the player and
            // kill them. (We need to drop the immutable references first, or we'd be mutably
            // borrowing the players when there are still immutable references around.)
            drop(living);

            // Get a mutable reference to the player who has been voted out.
            let voted = self.players.get_mut(&voted_id).unwrap();

            // Kill them.
            voted.dead = true;
        } else {
            self.send_all(&StcMessage::NoMajority);
        }

        // Count wolves and villagers to see if the game has ended.
        let (wolves, villagers) =
            self.players
                .values()
                .fold((0, 0), |(w, v), p| match p.role.unwrap() {
                    Role::Wolf => (w + 1, v),
                    Role::Villager => (w, v + 1),
                });

        if wolves == villagers {
            // If there are as many wolves as there are villagers, the wolves win.
            Some(Winner::Wolf)
        } else if wolves == 0 {
            // If the villagers have killed all the wolves, the village wins.
            Some(Winner::Village)
        } else {
            None
        }
    }

    /// Sends the given message to every player.
    fn send_all(&self, message: &StcMessage) {
        for player in self.players.values() {
            player.send(message);
        }
    }
}
