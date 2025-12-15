use std::collections::HashMap;
use std::net::{SocketAddr, Ipv4Addr};
use std::os::unix::net::SocketAddr;
use std::thread::spawn;
use async_trait::async_trait;
use bincode::{Decode, Encode};
use tokio_util::sync::CancellationToken;


use crate::network::{AsyncRegister, AsyncHandler,AsyncPayloadHandler}; 
use crate::block::{Block, Carrier, Digester};
use crate::executor::{Executor, ChainExecutor};
use crate::archive::Archiver;
use crate::hash:: HashValue;
use crate::herrors::{HError, logger_error, logger_error_with_error};
use crate::network::{UdpConnection, Message, Payload};
use crate::nodes::{Identity, SignHandle };
use crate::constants::{UDP_RECV_PORT, TIME_MS_FOR_UNP_RECV};
use crate::req_resp::{RequestWorker, WrokReceiver};




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
    ///the node must have an AsyncPayloadHandler to handle the message
    fn async_payload_handler(&self) -> Result<AsyncPayloadHandler, HError>;
}


use crate::constants::CHANNEL_CAPACITY;
use crate::req_resp::{self, RequestWorker};
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
    /// * 'msg' -the message to be signed.
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

    ///Default Implementation:
    /// 
    /// When a message is received from the network:
    ///  1. verify the message's signature
    ///  2. get the payload from the message
    ///  3. chose a handler to handle the payload, generate a response payload
    ///  4. sign the response message with the node's private key, then send back
    ///     to the network.
    /// This function creates a task to deliver the message to its next step task.
    /// # Arguments
    /// * `request_worker` - the worker to handle the request 
    /// * `response_worker` - the worker to send the response  back to the network
    /// * `cancel_token` - the cancellation token for the task
    /// 
    async fn spawn_deliver_message_task(
        &self,
        request_worker: RequestWorker<Payload>,
        response_worker: RequestWorker<(Vec<u8>, SocketAddr)>,
        cancel_token: CancellationToken,
    ) -> Result<(), HError> {
        let my_info = self.nodeinfo()?;
        let my_name = my_info.name()?;
        let bind_addrs = my_info.address()?;
        
        //Note: I just use the 1st address as a bind_addr for now.
        let bind_addr = bind_addrs[0].clone();

        //clone the sign_handle, so the new task can sign a message
        let sign_handle = self.sign_handle()?.clone();

        let task = async move {
            loop {
                tokio::select! {
                    // if we get a cancellation signal, break out of the loop
                    _ = cancel_token.cancelled() => {
                        break;
                    }
                    // if we get a message from the network, we create a task
                    // to handle the message
                    result = Self::udp_recv_from(
                        &my_name,
                        TIME_MS_FOR_UNP_RECV,
                        bind_addr,
                    ) => {
                        match result {
                            //it's an error message
                            Err(e) => {
                                logger_error_with_error(e);
                                break;
                            }
                            //It's an message not an error
                            Ok((msg, request_addr)) => {
                                //create a new task to handle the message, don't wait for the result.
                                tokio::spawn(
                                    helpers::msg_delivery(
                                        msg, &sign_handle, 
                                        request_worker.clone(), 
                                        response_worker.clone(), 
                                        request_addr
                                    )
                                );   
                            }
                        }
                    }
                }
            }
        };

        tokio::spawn(task);
        Ok(())
    }
        
    ///use the async_payload_handler to spawn new task to deal with 
    /// the request message's payload
    /// 
    /// # Returns
    /// * `Result<RequestWorker<Payload>, HError>` - we can create a Request with your
    /// Payload and this worker to this handler task.
    /// 
    /// # Example
    /// ```
    /// let request_worker = self.spawn_payload_handler_task.await?;
    /// let resp = Request::send(your_payload, requests_handler).await?;
    /// let returned_payload = resp.response().await?;
    /// ```

    async fn spawn_payload_handler_task(
        &self,
        cancel_token: CancellationToken,
    ) -> Result<RequestWorker<Payload>, HError>
    {
        //get the async_payload_handler from the node
        let async_payload_handler 
            = self.async_payload_handler()?;
        async_payload_handler.spawn_run(CHANNEL_CAPACITY, cancel_token)
            .await
    }


    /// Default Implmentation: 
    /// create a new task for sending the messages into network.
    async fn spawn_send_to_network_task(
        &self,
        addr: SocketAddr,
        cancel_token: CancellationToken,
    ) -> RequestWorker<(Vec<u8>, SocketAddr), HError> 
    {
        //get a bind_addr 
        let this_nodeinf = self.nodeinfo()?;
        let bind_addrs = this_nodeinf.address?;
        if bind_addrs.len() == 0 {
            return Err(
                HError::NetWork { message: 
                    format!("this node have no bind address in the node's nodeinfo") 
                }
            );
        }
        
        //just use the 1st address in the bind_addresses vector
        let bind_addr = bind_addrs[0].clone();

        let (request_worker, worke_receiver) = 
            req_resp::create_channel::<(Vec<u8>, SocketAddr)>(CHANNEL_CAPACITY);
        let task = async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        //break out of the loop
                        break;
                    }

                    (vec_bytes, addr) = work_receiver.recv_work() => {
                        //use the udp_connection methods
                        let result = Self::udp_send_to(dst_addr, msg, sign_handle).await;
                    };
                }   
            }
        };
        tokio::spawn(task);
        Ok(request_worker)
    }


    ///Default Implmentation:
    ///add a friend to the node's friends list
    /* 
    fn add(&mut self, name: HashValue, info: NodeInfo) -> Result<(), HError> {
        let friends = self.friends()?;
        friends.insert(name, info);
        Ok(())

    }
    */
    
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

///this module contains some helper functions for the Node trait
mod helpers {
    use super::*;
    use crate::constants::MAX_MSG_SIZE;
    use crate::nodes::SignHandle;
    use crate::req_resp::{Request, RequestWorker};
    use crate::herrors::HError;


    ///this function is the helper to deliver the message between the tasks
    /// 1. send the request message to the handler, by the request_worker,
    ///     wait for the result.
    /// 2. use the result to create a new message, in this process, the new
    ///     message will be encoded, signed, and add a header, it will be a 
    ///     real message can be transfered in the network.
    /// 3. deliver the msg_bytes and the target address to a sending task, which
    ///     will send the msg_bytes to the target address.
    pub async fn msg_delivery(
        msg: Message, 
        sign_handle: &SignHandle,
        request_worker: RequestWorker<Payload>,
        response_worker: RequestWorker<(Vec<u8>, SocketAddr)>,
        addr: SocketAddr,
    ) -> Result<(), HError> 
    {
        //send the message which we got from the network to the specific task,
        //we will get a new payload to generate a response message for the original
        //message that from the net.
        let payload = msg.payload;
        let response = Request::send(payload,  request_worker).await?;
        let new_payload = response.response().await?;


        //create a new message with this payload
        let new_reciver = msg.sender;
        let new_sender = msg.receiver;
        let new_msg = Message::new(new_sender, new_reciver, new_payload);
        
        //enocde the message (this will signated the message and add a header)
        let mut msg_encoded_byte = vec![0u8; MAX_MSG_SIZE]; 
        let msg_size = new_msg.encode(sign_handle, &mut msg_encoded_byte).await?;

        //truncate the vec
        msg_encoded_byte.truncate(msg_size);

        //send back the msg_byte to the response worker, that worker will send 
        //the message to the tartget.
        let _ = Request::send((msg_encoded_byte, addr), response_worker).await?;

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
async fn test_start_sign_handle() {     
        let cancel_token = CancellationToken::new();
        let mut node = TestNode::init_new(cancel_token.clone()).await.unwrap();
        let id = Identity::new();
        
        //bind a task to handle the sign request
        let result = node.start_sign_handle(cancel_token.clone(), id).await;
        assert!(result.is_ok());

        //sign a message then verify it
        let msg = b"hello world";
        let signature = node.sign(msg).await.unwrap();
        let result = node.sign_handle().unwrap().verify(msg, &signature);
        assert!(result.is_ok());
        
    }
}

