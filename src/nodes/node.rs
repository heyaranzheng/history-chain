use std::collections::HashMap;
use std::sync::Arc;
use std::net::{SocketAddr, Ipv4Addr};
use async_trait::async_trait;
use bincode::{Decode, Encode};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;


use crate::block::{Block, Carrier, Digester};
use crate::executor::{Executor, ChainExecutor};
use crate::archive::Archiver;
use crate::hash:: HashValue;
use crate::herrors::HError;
use crate::network::{UdpConnection, Message, Payload};
use crate::nodes::{Identity, SignHandle, SignRequest, identity};
use crate::constants::{UDP_RECV_PORT, TIME_MS_FOR_UNP_RECV};




///Note:
///     Nodeinfo is not common way to describe a node in the network.
///     It is needed if Node A wants to describe Node B, all the information in NodeInfo is 
/// for Node A, not for other nodes. 
///     Namely, the NodeInfo of Node B for Node A ONLY presents the perspectives of Node A to 
/// Node B, not the whole network's perspective.
#[derive(Debug, Clone, PartialEq, Decode, Encode)]
pub struct NodeInfo {
    ///the node's name, it is the public key of the node's identity
    pub name: Option<HashValue>,
    /// Node can have serveral usual addresses for connecting to the node,
    ///if we can't connect to the node by any of these addresses, we can try to connect to some 
    ///server to get the node's current address, if the node is in the network now.
    pub address: Option<Vec<SocketAddr>>,
    ///the caller, if we can't use those addresses to connect to the node, we can ask the caller
    pub caller: Option<HashValue>,
    ///the node's reputation for this instance's owner, not for whole the network.
    pub reputation: Option<Reputation>,

    // -------------private fields--------------
    
    ///the last time the node was connected to the network, in seconds.
    last_conn:  Option<u64>,
    ///the state of last connection
    last_state: Option<NodeState>,
    ///the number of times the node has been connected to the network
    meeting_count: Option<u32>,
    ///the successful conections rate of the node by the way using addresses
    conn_rate: Option<f32>,
    ///who introduced this node to me？
    introducer: Option<HashValue>,
}

impl NodeInfo {
    pub fn new() -> Self {
        NodeInfo {
            name: None,
            address: None,
            caller: None,
            reputation: None,
            last_conn: None,
            last_state: None,
            meeting_count: None,
            conn_rate: None,
            introducer: None,
        }
    }
    
    #[inline]
    pub fn name(&self) -> Result<HashValue, HError> {
        if let Some(name) = self.name {
            Ok(name)
        } else {
            Err(HError::Message {
                message: "node name is not set".to_string(),
            })
        }
    }

    #[inline]
    pub fn address(&self) -> Result<Vec<SocketAddr>, HError> {
        if let Some(address) = self.address.clone() {
            Ok(address)
        } else {
            Err(HError::Message {
                message: "node address is not set".to_string(),
            })
        }
    }

    #[inline]
    pub fn caller(&self) -> Result<HashValue, HError> {
        if let Some(caller) = self.caller {
            Ok(caller)
        } else {
            Err(HError::Message {
                message: "node caller is not set".to_string(),
            })
        }
    }

    #[inline]
    pub fn reputation(&self) -> Result<Reputation, HError> {
        if let Some(reputation) = self.reputation.clone() {
            Ok(reputation)
        } else {
            Err(HError::Message {
                message: "node reputation is not set".to_string(),
            })
        }
    }

    #[inline]
    pub fn last_conn(&self) -> Result<u64, HError> {
        if let Some(last_conn) = self.last_conn {
            Ok(last_conn)
        } else {
            Err(HError::Message {
                message: "node last_conn is not set".to_string(),
            })
        }
    }

    #[inline]
    pub fn last_state(&self) -> Result<NodeState, HError> {
        if let Some(last_state) = self.last_state.clone() {
            Ok(last_state)
        } else {
            Err(HError::Message {
                message: "node last_state is not set".to_string(),
            })
        }
    }
    
}

///trait for Node, it is used to operate the node's private fields,
/// especially for the sign_handle and nodeinfo.
pub trait NodeAppend {
    ///create a new node
    fn new() -> Self;
    ///set node's nodeinfo
    fn set_nodeinfo(&mut self, nodeinfo: NodeInfo);
    ///get node's nodeinfo
    fn nodeinfo(&self) -> Result<&NodeInfo, HError>;
    ///set node's sign_handle
    fn set_sign_handle(&mut self, sign_handle: SignHandle);
    ///get node's sign_handle
    fn sign_handle(&self) -> Result<&SignHandle, HError>;
    ///get node's friends
    fn friends(&self) -> Result<&HashMap<HashValue, NodeInfo>, HError>;
}


use crate::constants::CHANNEL_CAPACITY;
#[async_trait]
pub trait Node: UdpConnection + Sized + NodeAppend{       
    
    ///Defuault Implmentation:
    ///start the node's sign_handle task
    ///# Arguments
    /// * `cancel_token` - the cancellation token for the sign_handle task
    /// * `id` - the identity of the node, the id will be consumed to create the sign_handle
    /// # Example
    /// ```
    /// let sign_handle = node.start_sign_handle(cancel_token, id).await?;
    /// ```
    async fn start_sign_handle(&mut self, cancel_token: CancellationToken, id: Identity)
        -> Result<(), HError> {
        //create a new sign_handle
        let sign_handle = 
            SignHandle::spawn_new(id, CHANNEL_CAPACITY, cancel_token).await?;
        self.set_sign_handle(sign_handle);
        Ok(())
    }

    ///Default Implmentation:
    ///sign a message with the node's private key
    /// # Arguments
    /// * 'msg' - &[u8], the message to be signed.
    /// # Example
    /// ```
    /// let signature = node.sign(msg).await?;
    /// ```
    async fn sign(&self, msg: &[u8]) -> Result<[u8; 64], HError> {
        let sign_handle_ref = self.sign_handle()?;
        let sig = sign_handle_ref.sign(msg).await?;
        Ok(sig)
    }
  
   

    ///Default Implmentation:
    ///check if the node is a friend 
    fn is_friend(&self, name: HashValue) -> Result<bool,HError> {
        let friends = self.friends()?;
        Ok(friends.contains_key(&name))
    }

    /// Default Implmentation:
    /// Initialize a new node with a cancellation token
    async fn init_new(cancel_token: CancellationToken) -> Result<Self, HError>   {
        let mut node = Self::new();
        let id = Identity::new();
        let name = id.public_key_to_bytes();

        //create a new spwan_blocking task to handle the sign request
        let sign_handle = SignHandle::spawn_new(id, 32, cancel_token).await?;
        node.set_sign_handle(sign_handle);

        let mut node_info= NodeInfo::new();
        node_info.name = Some(name);
        node.set_nodeinfo(node_info);

        Ok(node)    
    }

    ///Default Implmentation: 
    //async fn sign_msg(&self, msg: u8) -> Result<[u8; 64], HError> 
    
    
    ///Default Implmentation:
    ///get a firend's info 
    fn get(&self, name: HashValue) -> Option<NodeInfo> {
        if let Ok(friends) = self.friends() {
            //this node has a friends list
            if let Some(info) = friends.get(&name) {
                return Some(info.clone());
            }
            //have no friend named  "name"
            return None;
        } else {
            //this node has no friends list
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
        //get my name
        let this_name_info = self.nodeinfo()?;
        let this_name = this_name_info.name()?;

        //check if the introducer is a friend
        let info = self.get(introducer);
        if info.is_none() {
            return Err(HError::Message {message: "introducer is not your friend".to_string()});
        }

        //get the introducer's info, and filter out the avaliable addresses of the introducer 
        let introducer_info = info.unwrap();
        let dst_addr = &introducer_info.address()?;

        let avaliable_addr = 
            Self::check_addresses_available(dst_addr, timeout_ms, introducer, sign_handle.clone()).await?;
        if avaliable_addr.is_empty() {
            return Err(HError::Message {message: "no avaliable address".to_string()});
        }

        //create a message for introducing a new node
        let msg = Message::new(
            this_name,
            introducer_info.name()?, 
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
        if src_addr != introducer_info.address()?[0] {
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
        //check if the introducer is a friend. If not, return error.
        if let Err(e) = self.is_friend(name) {
            return Err(e);
        }

        //--------------------------------
        //do something




        //--------------------------------
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
    use crate::nodes::identity::{SignHandle, SignRequest, Identity};

    struct TestNode {
        this_node_info: NodeInfo,
        sign_handle: Option<SignHandle>,
        friends: HashMap<HashValue, NodeInfo>,
    }

    impl NodeAppend for TestNode {
        fn set_nodeinfo(&mut self, nodeinfo: NodeInfo) {
            self.this_node_info = nodeinfo;
        }
        fn friends(&self) -> Result<&HashMap<HashValue, NodeInfo>, HError> {
            Ok(&self.friends)
        }
        fn nodeinfo(&self) -> Result<&NodeInfo, HError> {
            Ok(&self.this_node_info)
        }
        fn set_sign_handle(&mut self, sign_handle: SignHandle) {
            self.sign_handle = Some(sign_handle);
        }
        fn sign_handle(&self) -> Result<&SignHandle, HError> {
            if let Some(handle) = &self.sign_handle {
                Ok(handle)
            }else {
                Err(HError::Message {message: "sign_handle is not set".to_string()})
            }
        }

        fn new() -> Self {
            Self {
                this_node_info: NodeInfo::new(),
                sign_handle: None,
                friends: HashMap::new(),
            }
        }
    
    }

    impl UdpConnection for TestNode {}
    impl Node for TestNode {}
   
    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {     
        let cancel_token = CancellationToken::new();
        let node = TestNode::init_new(cancel_token).await.unwrap();
        let id = Identity::new();

        
    }
}

