use std::collections::HashMap;
use std::net::SocketAddr;
use async_trait::async_trait;


use crate::block::{Block, Carrier, Digester};
use crate::executor::{Executor, ChainExecutor};
use crate::archive::Archiver;
use crate::hash:: HashValue;
use crate::herrors::HError;
use crate::network::{UdpConnection, Message, Payload};
use crate::nodes::Identity;




///Note:
///     Nodeinfo is not common way to describe a node in the network.
///     It is needed if Node A wants to describe Node B, all the information in NodeInfo is 
/// for Node A, not for other nodes. 
///     Namely, the NodeInfo of Node B for Node A ONLY presents the perspectives of Node A to 
/// Node B, not the whole network's perspective.
#[derive(Debug, Clone)]
pub struct NodeInfo {
    name: HashValue,
    /// Node can have serveral usual addresses for connecting to the node,
    ///if we can't connect to the node by any of these addresses, we can try to connect to some 
    ///server to get the node's current address, if the node is in the network now.
    address: Vec<SocketAddr>,
    ///the caller, if we can't use those addresses to connect to the node, we can ask the caller
    caller: NodeName,
    ///the node's reputation for this instance's owner, not for whole the network.
    reputation: Reputation,
    ///the last time the node was connected to the network, in seconds.
    last_conn: u64,
    ///the state of last connection
    last_state: NodeState,
    ///the number of times the node has been connected to the network
    meeting_count: u32,
    ///the successful conections rate of the node by the way using addresses
    conn_rate: f32,
    ///who introduced this node to me？
    introducer: NodeName,

}



#[async_trait]
pub trait Node: UdpConnection{       
    ///get node's name
    fn name(&self) -> HashValue;
    ///get node's address
    fn address(&self) -> String;
    ///get node's friends
    fn friends(&self) -> &HashMap<HashValue, NodeInfo>;
    
    ///Default Implmentation:
    ///check if the node is a friend 
    fn is_friend(&self, name: HashValue) -> bool{
        self.friends().contains_key(&name)
    }

    ///Default Implmentation:
    ///get a firend's info 
    fn get(&self, name: HashValue) -> Option<NodeInfo>{
        let info = self.friends().get(&name);
        if let Some(info) = info {
            Some(info.clone())
        } else {
            None
        }
    }
    
    ///Default Implmentation:
    ///a node can introduce some nodes to his friend, if his friend wants to make more friends
    ///Those nodes which are introduced must have a good reputation (> 80) and active state.
    async fn make_new(&self, introducer: NodeName, timeout_ms: u64, identity: &mut Identity) -> Result<Vec<NodeInfo>, HError>{
        //check if the introducer is a friend
        let info = self.get(introducer);
        if info.is_none() {
            return Err(HError::Message {message: "introducer is not your friend".to_string()});
        }

        //get the introducer's info
        let introducer_info = info.unwrap();
        let msg = Message::new(
            self.name(), 
            introducer_info.name, 
            Payload::Introduce
        );

        let dst_addr = introducer_info.address;

        let avaliable_addr = Self::check_addresses_available(&dst_addr, timeout_ms, introducer, identity).await?;
        //send the message to the introducer
        let result = 
        Self::udp_send_to(avaliable_addr[0], &msg, identity).await?;
        
        Ok(Vec::new())
    }

    ///Default Implmentation:
    ///send a message to one of node's friend for introducing a new node
    async fn make_friend(&self, name: HashValue) -> Result<(), HError>{
        //check if the introducer is a friend. If not, return error
        if self.is_friend(name) {
            return Err(HError::Message {message: "introducer is not your friend".to_string()});
        }

        let msg = Message::new(
            self.name(), name, Payload::Introduce
        );

        Ok(())
    }

   
    async fn search_name(&self, name: HashValue) {
        
    }


    ///make a friend with the given node's name
    async fn make_friend_(&self, name: HashValue) -> Result<(), HError>{
        //check if the given node is already a friend, if it is, return Ok(())
        if self.is_friend(name) {
            return Ok(());
        }
        
        // can make them concurencey here!!!
        //get an intro list of node's name and its' address, between the two nodes,
        //including the   ddress.
        let intro_list= self.search_name(name).await?;

        //check the reputation of the introducer node, if it is good enough
        Ok(())



    }
}


///The reputaion of the node in the network.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

