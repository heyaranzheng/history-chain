use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, UdpSocket, TcpListener};
use std::net::SocketAddr;
use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::chain_manager::ChainManager;
use crate::constants::{
    MAX_CONNECTIONS , UDP_SENDER_PORT, UDP_RECV_PORT, TCP_SENDER_PORT, 
    TCP_RECV_PORT, MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, MTU_SIZE};
use crate::herrors::HError;
use crate::message::{Message, MessageType};
use crate::hash:: {HashValue, Hasher};


#[async_trait]
pub trait Node {       
    ///check if the node is the center node
    fn is_center(&self) -> bool;
    ///get a friend's node information by its name
    fn get_friend(&self, name: HashValue) -> Option<&UserNode>{None}
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

pub struct UserNode {
    ///name of the node
    pub name: NodeName,
    ///address of the node
    pub address: Option<String>,
    ///node's birthday
    pub timestamp: u64,
    ///friend nodes, HashMap<name, UserNode>
    pub friends: HashMap< NodeName, UserNode>,
    pub center_address: Option<String>,
    ///chain's manager
    pub chain_manager: Option<ChainManager>,
    ///node's status
    pub reputation: Reputation,
    ///node's state
    pub state: NodeState,
}

impl UserNode {
    pub fn new(name: NodeName, capacity: usize) -> Self {
        Self {
            name,
            address: None,
            timestamp: 0,
            friends: HashMap::with_capacity(capacity),
            center_address: None,
            chain_manager: None,
            reputation: Reputation::new(),
            state: NodeState::Sleepping,
        }
    }
}
 
#[async_trait]
impl Node for UserNode {
    fn is_center(&self) -> bool {
        false
    }
    
    //find a friend node's address by its name, return None if not found
    fn get_friend_address(&self, name: HashValue) -> Result<Option<String>, HError> {
        if let Some(friend) = self.friends.get(&name) {
            return Ok(friend.address.clone());
        }
        return Err(HError::Message { message: format!("this friend not found") });
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
    use crate::message::MessageType;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {
        let msg = Message {
            sender: HashValue::random(),
            timestamp: 0,
            message_type: MessageType::ChainRequest(0),
            receiver: HashValue::random(),
        };
        let save_msg = msg.clone();
        tokio::spawn(async move {
            let mut node = UserNode::new(HashValue::random(), 10);
            node.address = Some(format!("127.0.0.1:{}", UDP_SENDER_PORT));
            node.udp_send_to(format!("127.0.0.1:{}", UDP_RECV_PORT), &msg).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        });
      
        let node2 = UserNode::new(HashValue::random(), 10);
        let (msg , src_addr) = node2.udp_recv_from().await.unwrap();
        assert_eq!(msg, save_msg);
    }
}

