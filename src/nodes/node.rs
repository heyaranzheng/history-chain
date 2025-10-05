use std::collections::HashMap;
use async_trait::async_trait;


use crate::block::{Block, Carrier, Digester};
use crate::executor::{Executor, ChainExecutor};
use crate::archiver::Archiver;
use crate::hash:: HashValue;


#[async_trait]
pub trait Node {       
    ///check if the node is the center node
    fn is_center(&self) -> bool;
    ///get a friend's node information by its name
    fn get_friend(&self, name: HashValue) -> Option<&Self>{None}
    ///get node's name
    fn my_name(&self) -> HashValue;
    ///get node's address
    fn my_address(&self) -> Option<String>;
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

type NodeName = HashValue;

pub struct UserNode<B, D> 
    where B: Block + Carrier,
          D: Block + Digester
{
    ///name of the node
    pub name: NodeName,
    ///address of the node
    pub address: Option<String>,
    ///node's birthday
    pub timestamp: u64,
    ///friend nodes, HashMap<name, UserNode>
    pub friends: HashMap< NodeName, UserNode<B, D>>,
    pub center_address: Option<String>,
    ///chain's executor
    pub executor: Option<ChainExecutor<B, D>>,
    ///node's status
    pub reputation: Reputation,
    ///node's state
    pub state: NodeState,
}

impl <B, D> UserNode <B, D>
    where B: Block + Carrier,
          D: Block + Digester
{
    pub fn new(name: NodeName, capacity: usize) -> Self {
        Self {
            name,
            address: None,
            timestamp: 0,
            friends: HashMap::with_capacity(capacity),
            center_address: None,
            executor: None,
            reputation: Reputation::new(),
            state: NodeState::Sleepping,
        }
    }
}
 
#[async_trait]
impl <B, D> Node for UserNode<B, D>
    where B: Block ,
          D: Block + Digester 
{
    fn is_center(&self) -> bool {
        false
    }
    #[inline]
    fn my_address(&self) -> Option<String> {
        self.address.clone()
    }
    #[inline]
    fn my_name(&self) -> HashValue {
        self.name
    }
}
    


mod tests {
    use super::*;
    use crate::network::protocol::MessageType;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {
    
    }
}

