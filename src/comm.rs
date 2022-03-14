use serde::{Deserialize, Serialize};

/// The role of a player in the game.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum Role {
    Wolf,
    Villager,
}

/// The side that won when the game is over.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum Winner {
    Wolf,
    Village,
}

/// A unique identifier for a player within a room.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(usize);

impl PlayerId {
    pub fn new() -> PlayerId {
        PlayerId(0)
    }

    pub fn next(self) -> PlayerId {
        PlayerId(self.0 + 1)
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum CtsMessage {
    /// A message containing the player's name. This should be sent immediately after the client
    /// connects to the server. The server should reply with the player's ID.
    Connect(String),

    /// A vote against the player with the given index in the voting options that were sent to the
    /// player.
    Vote(usize),

    /// A wolf's chosen victim. The number is an index in the list of names they were sent.
    Kill(usize),

    /// Acknowledges receipt of a message from the server. The server should wait to receive this
    /// before sending any more messages to ensure that everything is sent in order.
    Received,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum StcMessage {
    /// The wolves have woken up and are going to vote on who to kill.
    WolvesWake,

    /// The game is entering a night.
    NightFalls,

    /// The name of the player who died last night.
    Died(String),

    /// The names of the players that can be voted against.
    VoteOptions(Vec<String>),

    /// The names of the players that can be killed by a wolf.
    KillOptions(Vec<String>),

    /// Player A has voted against player B.
    AnnounceVote(String, String),

    /// There was not a majority on the vote.
    NoMajority,

    /// The name of the player who was just voted out.
    VotedOut(String),

    /// The role assigned to the recipient player.
    RoleAssigned(Role),

    /// A side has won the game.
    AnnounceWinner(Winner),

    /// The host is waiting for a player to vote.
    WaitingFor(String),

    /// The ID-username pair for a player that just joined the game.
    AnnounceJoin(PlayerId, String),

    /// The ID assigned to a player who just joined.
    IdAssigned(PlayerId),

    /// An ID-username pair that identifies a certain player that is already in the game.
    PlayerData(PlayerId, String),
}
