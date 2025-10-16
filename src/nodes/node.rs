use std::collections::HashMap;
use async_trait::async_trait;


use crate::block::{Block, Carrier, Digester};
use crate::executor::{Executor, ChainExecutor};
use crate::archive::Archiver;
use crate::hash:: HashValue;
use crate::herrors::HError;

pub struct Route {
    name: HashValue,
    address: String,
}

impl Route {
    pub fn new(name: HashValue, address: String) -> Self {
        Self {
            name,
            address,
        }
    }
}

#[async_trait]
pub trait Node {       
    ///check if the node is the center node
    fn is_center(&self) -> bool;
    ///get node's name
    fn my_name(&self) -> HashValue;
    ///get node's address
    fn my_address(&self) -> Option<String>;
    ///check if the node is a friend 
    fn is_friend(&self, name: HashValue) -> bool;

    ///get a path or a route from one self node to the target node.
    ///it's a list of node's name and its' address, between the two nodes.
    ///Form example:
    /// A wants to find D, B is one of A's friend, C is another friend of B, D is the target node.
    /// A's path or a route to D is [A, B, C, D]
    async fn search_name(&self, name: HashValue) -> Result< Route, HError>{
        
    }


    ///make a friend with the given node's name
    async fn make_friend(&self, name: HashValue) -> Result<(), HErrror>{
        //check if the given node is already a friend, if it is, return Ok(())
        if self.is_friend(name) {
            return Ok(());
        }
        
        // can make them concurencey here!!!
        //get an intro list of node's name and its' address, between the two nodes,
        //including the target node's name and its' address.
        let intro_list= self.search_name(name).await?;

        //check the reputation of the introducer node, if it is good enough
        Ok(())



    }
}


///The reputaion of the node in the network.
pub struct Reputation {
    ///node's reputation score, default is 0
    score: u8,
}
impl Reputation {
    pub fn new() -> Self {
        Self {
            score: 60,
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
    


mod tests {
    use super::*;
    use crate::network::protocol::MessageType;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {
    
    }
}

