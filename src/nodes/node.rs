use std::collections::HashMap;
use std::sync::Arc;
use std::net::{SocketAddr, Ipv4Addr};
use async_trait::async_trait;
use bincode::{Decode, Encode};
use tokio::sync::Mutex;


use crate::block::{Block, Carrier, Digester};
use crate::executor::{Executor, ChainExecutor};
use crate::archive::Archiver;
use crate::hash:: HashValue;
use crate::herrors::HError;
use crate::network::{UdpConnection, Message, Payload};
use crate::nodes::{Identity, identity,SignHandle};
use crate::constants::{UDP_RECV_PORT, TIME_MS_FOR_UNP_RECV};




///Note:
///     Nodeinfo is not common way to describe a node in the network.
///     It is needed if Node A wants to describe Node B, all the information in NodeInfo is 
/// for Node A, not for other nodes. 
///     Namely, the NodeInfo of Node B for Node A ONLY presents the perspectives of Node A to 
/// Node B, not the whole network's perspective.
#[derive(Debug, Clone, PartialEq, Decode, Encode)]
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
    fn address(&self) -> SocketAddr;
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
    async fn make_new(
        &self, introducer: NodeName, 
        timeout_ms: u64, 
        sign_handle: SignHandle,
    ) -> Result<NodeInfo, HError>{
        //check if the introducer is a friend
        let info = self.get(introducer);
        if info.is_none() {
            return Err(HError::Message {message: "introducer is not your friend".to_string()});
        }

        //get the introducer's info, and filter out the avaliable addresses of the introducer 
        let introducer_info = info.unwrap();
        let dst_addr = &introducer_info.address;

        let avaliable_addr = 
            Self::check_addresses_available(dst_addr, timeout_ms, introducer, sign_handle.clone()).await?;
        if avaliable_addr.is_empty() {
            return Err(HError::Message {message: "no avaliable address".to_string()});
        }

        //create a message for introducing a new node
        let msg = Message::new(
            self.name(), 
            introducer_info.name, 
            Payload::Introduce
        );

        //send the message to the introducer，then wait for the response
        let _ = Self::udp_send_to(avaliable_addr[0], &msg, sign_handle.clone()).await?;

        //bind a temporary ip to receive the response
        let bind_addr = SocketAddr::new(
            Ipv4Addr::new(0, 0, 0, 0).into(), UDP_RECV_PORT
        );

        let (msg, src_addr) = 
            Self::udp_recv_from(&sign_handle.public_key_bytes(), TIME_MS_FOR_UNP_RECV, bind_addr).await?;
        
        //check if the response is valid
        if src_addr != introducer_info.address[0] {
            return Err(HError::Message {
                message: "response is not from the introducer".to_string()});
        }

        //get out the node's info from the response
        let payload = msg.payload;
        if let Payload::IntroduceResp(new_node_info) = payload {
            Ok(new_node_info)
        }else {
            Err(HError::Message {
                message: "introduce response for a new node is not valid".to_string()
            }) 
        }
        
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
}


///The reputaion of the node in the network.
#[derive(Debug, Clone, PartialEq, Decode, Encode)]
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
#[derive(Debug, Clone, PartialEq, Decode, Encode)]
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {
    
    }
}

