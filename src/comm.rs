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

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum CtsMessage {
    /// The player's name. This should be sent immediately after a connection is made.
    Name(String),

    /// A vote against the player with the given index in the voting options that were sent to the
    /// player.
    Vote(usize),

    /// A wolf's chosen victim. The number is an index in the list of names they were sent.
    Kill(usize),
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
}
