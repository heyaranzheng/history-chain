use std::collections::HashMap;
use std::net::{SocketAddr, Ipv4Addr};
use std::sync::Arc;
use async_trait::async_trait;
use bincode::{Decode, Encode};
use tokio_util::sync::CancellationToken;
use tokio::task::spawn;

use crate::constants::CHANNEL_CAPACITY;
use crate::network::{AsyncHandler, AsyncPayloadHandler, AsyncRegister, udp_send_to}; 
use crate::block::{Block, Carrier, Digester};
use crate::executor::{Executor, ChainExecutor};
use crate::archive::Archiver;
use crate::hash:: HashValue;
use crate::herrors::{HError, logger_error, logger_error_with_error, self};
use crate::network::{UdpConnection, Message, Payload};
use crate::nodes::{Identity, SignHandle };
use crate::constants::{UDP_RECV_PORT, TIME_MS_FOR_UNP_RECV, MAX_UDP_MSG_SIZE};
use crate::req_resp::{self, RequestWorker, WorkReceiver, create_channel, Request};




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
#[async_trait]
pub trait NodeAppend {
    ///create a new node
    async fn new() -> Self;
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
        let mut node = Self::new().await;
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
    /// Step 1: 
    async fn spawn_recv_net_task(
        &self,
        payload_worker: RequestWorker<(Message, SocketAddr)>,
        cancel_token: CancellationToken,
    ) -> Result<(), HError> {
        let my_info = self.nodeinfo()?;
        let my_name = my_info.name()?;
        let bind_addrs = my_info.address()?;
        
        //Note: I just use the 1st address as a bind_addr for now.
        let bind_addr = bind_addrs[0].clone();

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
                                logger_error_with_error(&e);
                                break;
                            }
                            //It's an message not an error
                            Ok((msg, request_addr)) => {
                                //send the message to the next processing task.
                                let _ = Request::send(
                                    (msg, request_addr), 
                                    payload_worker.clone()
                                ).await;

                            }
                        }
                    }
                }
            }
        };

        tokio::spawn(task);
        Ok(())
    }
        


    ///Default Implementation:
    /// Step 2: 
    ///     find a handler in the AsyncPayloadHandler, then spawn a chiild task to handle it.
    /// the result of the child task will be sent to the next step task -- encode_and_sign_task
    /// 
    /// #Arguments
    /// *`encode_and_sign_worker` - the RequestWroker  of the next step task
    /// *`cancel_token` - the cancellation token for the task
    /// 
    /// # Returns
    /// * `Result<RequestWorker<(Message, SocketAddr)>, HError>` - the RequestWorker of this 
    /// processing task. You can use this worker to construct a Request and send it to  this task.
    /// # Example
    /// ```
    /// let request_worker = self.spawn_hanler_task.await?;
    /// //we don't send back to requester, so the returns of the Response will not be used. 
    /// let _ = Request::send(your_payload, requests_handler).await?;
    /// ```
    async fn spawn_handler_task(
        &self,
        encode_and_sign_worker: RequestWorker<(Message, SocketAddr)>,
        cance_token: CancellationToken,
    ) -> Result<RequestWorker<(Message, SocketAddr)>, HError> 
    {

        //get the async_payload_handler from the node, and share it to all the child tasks.
        let async_payload_handler = self.async_payload_handler()?;
        let async_payload_handler_share = Arc::new(async_payload_handler);

        let (request_worker, mut work_receiver) 
            = create_channel::<(Message, SocketAddr)>(CHANNEL_CAPACITY);
   
        let task = async move {
            loop {
                //clone the encode_and_sign_worker, so the new task can send a message
                let encode_and_sign_worker_clone = encode_and_sign_worker.clone();
                tokio::select! {
                    _ = cance_token.cancelled() => {
                        break;
                    }
               
                    //wait for a request
                    msg_addr_result = work_receiver.recv_data() => {
                        match msg_addr_result {
                            Err(e) => {
                                logger_error_with_error(&e);
                                continue;
                            }
                            
                            Ok((msg, request_addr)) => {
                                let async_payload_handler_share_clone = 
                                    async_payload_handler_share.clone();
            
                                //create a child task to handle the message, this task just handle the
                                //message, then send it to the next step task, and finish.
                                let child_task = async move {
                                    let msg_addr = (msg, request_addr);
                                    let result = 
                                        async_payload_handler_share_clone.spawn_handle(
                                            msg_addr, 
                                            encode_and_sign_worker_clone
                                        ).await;
                                    //if there is an error, log it.
                                    if let Err(e) = result {
                                        logger_error_with_error(&e);
                                    };

                                };

                                //spawn the task, don't wait for it to finish.
                                tokio::spawn(child_task);
                            }
                        }
                    }
                }

            }
        };
        
        //don't  wait for the task to finish, just spawn it.
        tokio::spawn(task);
        Ok(request_worker)

    }

    ///Default Implementation:
    /// Step 3:
    ///     encode the message and sign it.
    ///     send the message to the next step task -- spawn_udp_send_to_task
    /// # Arguments
    /// `send_to_worker` - the RequestWorker of the next step task (step 4)
    /// * `cancel_token` - the cancellation token to cancel the task.
    /// # Returns
    /// * `Result<RequestWorker<(Message, SocketAddr)>, HError>` - a request worker make a request
    /// for this task. This RequestWorker will be used in Step 3 to send a message to  this task.

    async fn spawn_encode_and_sign_task(
        &self,
        send_to_worker: RequestWorker<(Vec<u8>, SocketAddr)>,
        cancel_token: CancellationToken,
    ) -> Result<RequestWorker<(Message, SocketAddr)>, HError> 
    {
        let sign_handle = self.sign_handle()?.clone();
        let (request_worker, mut work_receiver) 
            = create_channel::<(Message, SocketAddr)>(CHANNEL_CAPACITY);

        let task = async move {
            loop {

                let send_to_worker_clone = send_to_worker.clone();
                tokio::select! {
                    //get a cancellation signal, break out of the loop
                    _ = cancel_token.cancelled() => {
                        break;
                    }
                    //wait for a request
                    msg_addr_result = work_receiver.recv_data() => {
                        match msg_addr_result {
                            Err(e) => {
                                logger_error_with_error(&e);
                                continue;
                            }
                            Ok((msg, request_addr)) => {
                                //encode the message and sign it, then add a header to the message
                                let mut vec_bytes = vec![0u8; MAX_UDP_MSG_SIZE];
                                let msg_bytes_size_result = msg.encode(&sign_handle, &mut vec_bytes[..])
                                    .await;
                                match msg_bytes_size_result {
                                    Err(e) => {
                                        logger_error_with_error(&e);
                                        continue;
                                    }
                                    //if it is ok, truncate the vector to the size of the message.
                                    //send it to the next step task
                                    Ok(msg_bytes_size) => {
                                        vec_bytes.truncate(msg_bytes_size);

                                        //send the vec_bytes to the next step task
                                        let result = Request::send(
                                            (vec_bytes, request_addr), 
                                            send_to_worker_clone
                                        ).await;
                                        //if send failed, log it.
                                        if let Err(e) = result {
                                            logger_error_with_error(&e);
                                        };
                                    }
                                
                                }
                            }
                        }
                    }
                }
            }
        };
        //don't  wait for the task to finish, just spawn it.
        tokio::spawn(task);


        Ok(request_worker)
    }


    ///Default Implementation:
    /// Step 4:
    ///Send a udp message to a destination address.
    /// # Arguments
    /// * `bind_addr` - the address to bind to.
    /// * `cancel_token` - the cancellation token to cancel the task.
    /// # Returns
    /// * `Result<RequestWorker<(Vec<u8>, SocketAddr)>, HError>` - a request worker make a request
    /// for this task.
    /// 
    /// # Example
    /// ```
    /// let request_worker = spawn_udp_send_to_task(bind_addr, cancel_token).await?;
    /// let _ = req_resp::Request::send((msg_bytes, dst_addr), &request_worker).await?;
    /// ```
    /// then the task will receive the request .
    async fn spawn_udp_send_to_task(
        &self,
        bind_addr: SocketAddr,
        cancel_token: CancellationToken,
    ) -> Result<RequestWorker<(Vec<u8>, SocketAddr)>, HError>
    {
        //create a channel to receive requests from other tasks
        let (requester, mut receiver) = 
            create_channel::<(Vec<u8>, SocketAddr)>(CHANNEL_CAPACITY);

        let task = async move {
            loop {
                let bind_addr_clone = bind_addr.clone();
                tokio::select! {
                    //wait for a request from other tasks
                    req_result = receiver.recv_data() => {
                        match req_result {
                            Ok(req_result) => {
                                let (msg_bytes, dst_addr) = req_result;
                                
                                //send the message to dst_addr
                                let result = udp_send_to(
                                    bind_addr_clone,
                                    dst_addr,
                                    &msg_bytes[..],
                                ).await;
                                match result {
                                    Ok(size) => {
                                        //send success, log it
                                        let ip = dst_addr.ip();
                                        let port = dst_addr.port();

                                        herrors::logger_info(
                                            &format!(
                                                "udp_send_to_task: send to {}:{} size: {}", ip , port, size
                                            )    
                                        );
                                        continue;
                                    },
                                    Err(e) => {
                                       //send failed, log it
                                       herrors::logger_error_with_error(&e);
                                       continue;
                                    }
                                }
                            },
                            Err(e) => {
                                //bad request, log it
                                herrors::logger_error_with_error(&e);
                                continue;
                            }
                        }
                    },
                    _ = cancel_token.cancelled() => {
                        //got an exitting signal
                        break;
                    }
                }
            }
        };
        tokio::task::spawn(task);
        Ok(requester)
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
        let _ = Self::udp_send_to(avaliable_addr[0], &msg, &sign_handle.clone()).await?;

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
    /// 
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
    
#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;
    use crate::nodes::identity::{SignHandle, SignRequest, Identity};
    use crate::herrors::HError;
    use crate::req_resp::{Request, RequestWorker};
    use crate::network::{Payload, PayloadTypes};

    struct TestNode {
        this_node_info: NodeInfo,
        sign_handle: Option<SignHandle>,
        friends: HashMap<HashValue, NodeInfo>,
        async_payload_handler: AsyncPayloadHandler,
    }

    #[async_trait]
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
        fn async_payload_handler(&self) -> Result<AsyncPayloadHandler, HError> {
            Ok(self.async_payload_handler.clone())
        }

        ///create a test node with a sign handle and a async payload handler
        ///We give 2 bind address to the node, one is bound to the listen port 8080,
        ///the other one is bound to the sender port 8888
        async fn new() -> Self {
            //crate a  nodeinfo
            let mut nodeinfo = NodeInfo::new();

            //give this node two addresses.
            let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
            let bind_addr = 
                SocketAddr::new(ip, 8080);
            let sender_addr = SocketAddr::new(ip, 8888);
            let vec_addr = vec![bind_addr, sender_addr];
            nodeinfo.address= Some(vec_addr);

            //create a identity, and use it to create a sign handle
            let id = Identity::new();
            let sign_handle_result = 
                SignHandle::spawn_new(id, 32, CancellationToken::new())
                .await;
            assert_eq!(sign_handle_result.is_ok(), true);
            let sign_handle = sign_handle_result.unwrap();
            let name = sign_handle.public_key_bytes();
            nodeinfo.name = Some(name);

            //create a handler for the Payload::Empty and register it to the node
            let async_handler = 
            |payload: Payload| async {
                herrors::logger_info("hello, it is handled");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                Ok::<Payload, HError>(payload)
            };
            let mut async_payload_handler = AsyncPayloadHandler::new();
            let result = async_payload_handler.reg_async(PayloadTypes::Empty, async_handler).await;
            assert!(result.is_ok());
            
            Self {
                this_node_info: nodeinfo,
                sign_handle: Some(sign_handle),
                friends: HashMap::new(),
                async_payload_handler: async_payload_handler,
            }
        }
    
    }

    #[async_trait]
    impl UdpConnection for TestNode {}
    #[async_trait]
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

    use tokio::net::UdpSocket;
    use crate::constants::MAX_UDP_MSG_SIZE; 

    ///create a new identity, and use it to sign  a empty message, then send it to the destination address.
    /// The destination address is set as 127.0.0.1:8080
    /// # Arguments
    /// * `name` - the name of the node to receive the message.
    /// * `cancel_token` - the cancellation token to cancel the task.
    async fn send_empty_payload_to_port_8080(name: HashValue, cancel_token: CancellationToken) 
        -> Result<(UdpSocket, HashValue), HError> {
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let sender_addr = SocketAddr::new(ip, 8081);

        //create a new id, which will give us a name. 
        //and we can use this id to sign the message.
        let id = Identity::new();
        let new_name = id.public_key_to_bytes();
        let sender_sign_handle = 
            SignHandle::spawn_new(id, 32, cancel_token.clone()).await?;

        //create a empty message 
        let msg = Message::new(new_name, name, Payload::Empty);
        let mut buffer = vec![0u8; MAX_UDP_MSG_SIZE];
        let bytes_size = msg.encode(&sender_sign_handle, &mut buffer).await.unwrap();
        buffer.truncate(bytes_size);

        let receiver_addr = SocketAddr::new(ip, 8080);
        //send the message to the test node
        let result = UdpSocket::bind(sender_addr).await;
        assert_eq!(result.is_ok(), true);
        let socket = result.unwrap();
        socket.send_to(&buffer, receiver_addr).await?;
        Ok((socket, new_name))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_step1_task()  {
        herrors::logger_init();
        let cancel_token = CancellationToken::new();
        let node = TestNode::new().await;
        let name = node.nodeinfo().unwrap().name().unwrap();

        //create a test channel to receive the request
        let (requester, mut receiver) = 
            create_channel::<(Message, SocketAddr)>(100);

        let result = node.spawn_recv_net_task(requester, cancel_token.clone()).await;
        assert_eq!(result.is_ok(), true);

        send_empty_payload_to_port_8080(name, cancel_token.clone()).await.unwrap();

        let result = receiver.recv_data().await;
        assert_eq!(result.is_ok(), true);
        let (msg, src_addr) = result.unwrap();
        assert_eq!(msg.payload, Payload::Empty);
        assert_eq!(src_addr.to_string(), "127.0.0.1:8081" );

    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_step2_task() {
        herrors::logger_init_above_info();
        let cancel_token = CancellationToken::new();
        let node = TestNode::new().await;
        let name = node.nodeinfo().unwrap().name().unwrap();

        //create a test channel to receive the request. we can send a data to this 
        //new task with requester and receive the result of the data processing from this 
        //task with receiver.
        let (requester, mut receiver) = 
            create_channel::<(Message, SocketAddr)>(100);
        
        //spawn step 2:
        let result = 
            node.spawn_handler_task(requester, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);
        let step_2_worker = result.unwrap();

        //spawn step 1:
        let result = 
            node.spawn_recv_net_task(step_2_worker, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);

        //create a new identity, send a empty payload message to the node
        let result = 
            send_empty_payload_to_port_8080(name, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);
        let (socket, new_name) = result.unwrap();

        //wait for the production of step2 task, after it process an empty payload message,
        let result = receiver.recv_data().await;
        assert_eq!(result.is_ok(), true);
        let (msg, src_addr) = result.unwrap();
        assert_eq!(msg.payload, Payload::Empty);
        assert_eq!(src_addr.to_string(), "127.0.0.1:8081" );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_step3_task() {
        herrors::logger_init_above_info();
        let cancel_token = CancellationToken::new();
        let node = TestNode::new().await;
        let name = node.nodeinfo().unwrap().name().unwrap();

        //create a test channel to receive the request. we can send a data to this 
        //new task with requester and receive the result of the data processing from this 
        //task with receiver.
        let (requester, mut receiver) = 
            create_channel::<(Vec<u8>, SocketAddr)>(100);
        
        //spawn step 3:
        let result = 
            node.spawn_encode_and_sign_task(requester, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);
        let step_3_worker = result.unwrap();

        //spawn step 2:
        let result = 
            node.spawn_handler_task(step_3_worker, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);
        let step_2_worker = result.unwrap();

        //spawn step 1:
        let result = 
            node.spawn_recv_net_task(step_2_worker, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);

        //create a new identity, send a empty payload message to the node
        let result = 
            send_empty_payload_to_port_8080(name, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);
        let (socket, new_name) = result.unwrap();

        //wait for the production of step3 task, after it process an empty payload message,
        let result = receiver.recv_data().await;
        assert_eq!(result.is_ok(), true);
        let (vec_bytes, src_addr) = result.unwrap();

        //get out the message from the vec_bytes, then verify the message and the src_addr
        let msg = Message::decode_from_slice(&new_name, &vec_bytes).unwrap();
        assert_eq!(msg.payload, Payload::Empty);
        assert_eq!(src_addr.to_string(), "127.0.0.1:8081" );
    }


    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_step4_task() {

        herrors::logger_init_above_info();

        let cancel_token = CancellationToken::new();

        let node = TestNode::new().await;
        let mut bind_addr = node.nodeinfo().unwrap().address().unwrap()[0];
        
        
        //the default listen port to receive the message of the node is 8080, so we need to use another
        //port to send  the message out from the node.
        bind_addr.set_port(8888);

        //spawn step 4:
        let result = 
            node.spawn_udp_send_to_task(bind_addr, cancel_token.clone()).await;
        assert_eq!(result.is_ok(), true);
        let step_4_worker = result.unwrap();

        //spawn step 3:
        let result = 
            node.spawn_encode_and_sign_task(step_4_worker, cancel_token.clone())
            .await;
        assert_eq!(result.is_ok(), true);
        let step_3_worker = result.unwrap();

        //spawn step 2:
        let result = node.spawn_handler_task(step_3_worker, cancel_token.clone()).await;
        assert_eq!(result.is_ok(), true);
        let step_2_worker = result.unwrap();

        //spawn step 1:
        let result = node.spawn_recv_net_task(step_2_worker, cancel_token.clone()).await;
        assert_eq!(result.is_ok(), true);

        //send a message to the test node
        let name = node.nodeinfo().unwrap().name().unwrap();
        let result =send_empty_payload_to_port_8080(name, cancel_token.clone()).await;
        assert_eq!(result.is_ok(), true);
        let (socket , new_name) = result.unwrap();

        let mut buffer = vec![0u8; MAX_UDP_MSG_SIZE];
        //wait for the response
        let (bytes_size, _) = socket.recv_from(&mut buffer).await.unwrap();
        
        let msg = Message::decode_from_slice(&new_name, &buffer[..bytes_size]).unwrap();
        assert_eq!(msg.payload, Payload::Empty);       
        cancel_token.cancel(); 
    }

}

