use std::fmt::Result;
use std::net::{TcpStream, UdpSocket};
use std::thread::spawn;

use crate::chain_manager::ChainManager;
use crate::constants::Hash;
use crate::herrors::HError;
use crate::message::Message;


pub trait Node {
    ///this node is a center?
    fn is_center(&self) -> bool;

    ///udp connection 
    fn udp_listen(&self) -> Result<(), HError>{
        let socket = UdpSocket::bind(localaddrress).unwrap();
        let socket_handle = sq    
    }
    ///tcp connection
    fn tcp_listen(&self) -> Result<TcpStream, HError>;
    
}


///The reputaion of the node in the network.
pub struct Reputation {
    ///node's reputation score, default is 0
    pub score: u8,
}
impl Reputation {
    pub fn new() -> Self {
        Self {
            score: 0,
        }
    }
}

/// The state of the node in the network. It is determined by the node itself.
/// The Sleeping state is the initial state of the node.
pub enum NodeState {
    ///node is active and free to communicate
    Active,
    ///node is active, but busy, it may need some time to deal with other nodes' requests
    Busy,                                                            
    ///node is inactive, disconnected from the network, 
    ///the default state of the node after it is created
    Sleepping,
}



pub struct UserNode {
    ///name of the node
    pub name: Hash,
    ///address of the node
    pub address: Option<String>,
    ///node's birthday
    pub timestamp: u64,
    ///friend nodes, new with a parameter to set the capacity of the friends list
    pub friends: Vec<UserNode>,
    ///data center's location
    pub center_address: Option<String>,
    ///chain's manager
    pub chain_manager: Option<ChainManager>,
    ///node's status
    pub reputation: Reputation,
    ///node's state
    pub state: NodeState,

}

impl Node {
    pub fn new(name: Hash, capacity: usize) -> Self {
        Self {
            name,
            address: None,
            timestamp: 0,
            friends: Vec::with_capacity(capacity),
            center_address: None,
            chain_manager: None,
            reputation: Reputation::new(),
            state: NodeState::Sleepping,
        }
    }
}




