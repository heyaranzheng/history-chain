use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, UdpSocket, TcpListener};
use std::net::SocketAddr;
use std::collections::HashMap;
use std::pin::Pin;
use async_trait::async_trait;
use tokio::sync::mpsc::{Sender, Receiver};

use crate::chain_manager::ChainManager;
use crate::constants::{ UDP_PORT, TCP_PORT, MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, MTU_SIZE};
use crate::herrors::HError;
use crate::message::Message;
use crate::hash:: {HashValue, Hasher};


#[async_trait]
pub trait Node {
    ///this node is a center?
    fn is_center(&self) -> bool;
    ///get a friend node's address by its name
    fn get_friend_address(&self, name: HashValue) -> Result<Option<String>, HError>;
    ///get my address
    fn my_address(&self) -> Option<String>;
    ///get my name
    fn my_name(&self) -> HashValue;

    ///udp connection, receive message from all nodes, return message and source address
    async fn udp_recv_from(&self) -> Result< (Message, String), HError> {
        let mut buf = vec![0; MAX_UDP_MSG_SIZE];
 
        //bind to udp port for any address
        let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_PORT)).await?;
        //receive data from udp socket
        let (size, src_addr) = udp_socket.recv_from(&mut buf).await?;
        let src_addr_str = format!("{}", src_addr.ip());
        
        //deserialize data to message
        let msg = Message::decode_from_slice(&buf[..size])?;

        Ok((msg, src_addr_str))
    }
    ///udp connection, send message to another node
    async fn udp_send_to(&self, dst_addr: String, msg: &Message) -> Result<usize, HError> 
    
    {

        let msg_encoded = msg.encode_to_vec()?;
        //check the buffer size
        if msg_encoded.len() > MAX_UDP_MSG_SIZE {
            return Err(
                HError::NetWork { 
                    message: format!("buffer size is too large, need at most {}", MAX_UDP_MSG_SIZE)
                } 
            );
        }
        //bind self address and udp port
        let my_address = self.my_address();
    
        if my_address.is_none() {
            return Err(HError::NetWork{ message: format!("address is not set") });
        }
        let udp_socket = 
            UdpSocket::bind(format!("{}:{}", my_address.unwrap(), UDP_PORT)).await?;

        //send data to dst_addr
        udp_socket.send_to(&msg_encoded[..], dst_addr).await;
        Ok(msg_encoded.len())
    }


    ///tcp connection
    async fn tcp_listen(&self) ->
        (Sender<bool>, Receiver<Vec<u8>>, tokio::task::JoinHandle<Result<(), HError>>) {
        //crate mpsc channel for tcp connection
        let (mut sender, mut receiver ) = 
            tokio::sync::mpsc::channel(MAX_MSG_SIZE);
        
        let (mut closer, mut switcher_receiver) 
            = tokio::sync::mpsc::channel::<bool>(1);
        

        let handle: tokio::task::JoinHandle<Result<(), HError>> = tokio::task::spawn( async move{
            //bind to tcp port for any address
            let tcp_listener = TcpListener::bind(format!("0.0.0.0:{}", TCP_PORT)).await?;
            loop {
                //check if the switcher is closed
                let signal =switcher_receiver.try_recv();
                match signal {
                    Ok(close) => {
                        //receive a true, close the tcp listener
                        if close == true  {
                            break;
                        }
                        //not close, continue to accept tcp connection
                    }
                    Err(_) => {
                        //no signal, continue to accept tcp connection
                    }
        
                }
                //accept tcp connection
                let mut tcp_stream = tcp_listener.accept().await?.0;
                let mut buf = vec![0; MTU_SIZE];
                while tcp_stream.read(&mut buf[..]).await? != 0 {
                    sender.send(buf).await
                        .map_err(|e| 
                            HError::Message { message: format!("send error: {}", e) }
                        )?;
                    buf = vec![0; MTU_SIZE];
                }
            }
            Ok(())
        });

        ( closer, receiver, handle ) 
    }
    //send message to another node, use udp
   
        

    
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
    fn get_address(&self) -> Option<String> {
        self.address.clone()
    }
}
    


mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {
        let node = UserNode::new(HashValue::random(), 10);
        let msg = Message {
            hash: HashValue::random(),
            sender: HashValue::random(),
            timestamp: 0,
            message_type: MessageType::Request,
            receiver: HashValue::random(),
        };
        let save_msg = msg.clone();
        tokio::spawn(async move {
           let mut node = UserNode::new(HashValue::random(), 10);
           node.address = Some("127.0.0.1:8080".to_string());
           node.udp_send_to(format!("127.0.0.1:8081"), &msg).await.unwrap();
        });
        let node2 = UserNode::new(HashValue::random(), 10);
        let (msg , src_addr) = 

    }
}

