use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, UdpSocket, TcpListener};
use std::net::SocketAddr;
use std::pin::Pin;
use async_trait::async_trait;
use tokio::sync::mpsc::{Sender, Receiver};

use crate::chain_manager::ChainManager;
use crate::constants::{Hash, UDP_PORT, TCP_PORT, MAX_MSG_SIZE, MAX_UDP_MSG_SIZE};
use crate::herrors::HError;
use crate::message::Message;

#[async_trait]
pub trait Node {
    ///this node is a center?
    fn is_center(&self) -> bool;
    ///get a friend node's address by its name
    fn get_friend_address(&self, name: Hash) -> Option<String>;

    ///udp connection, receive message from all nodes, return message and source address
    async fn upd_recv_from(&self, buf: &mut [u8]) -> Result<(Message, SocketAddr), HError> {
        //check  the buffer size
        if buf.len() < MAX_UDP_MSG_SIZE {
            return Err(HError::RingBuf { message: format!("buffer size is too small, need at least {}", MAX_UDP_MSG_SIZE) });
        }
        //bind to udp port for any address
        let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_PORT)).await?;
        //receive data from udp socket
        let (_, src_addr) = udp_socket.recv_from(buf).await?;
        
        //deserialize data to message
        let msg = Message::decode_from_slice(buf)?;

        Ok((msg, src_addr))
    }
    ///tcp connection
    async fn tcp_listen(&self) -> Result<Receiver, HError> {
        //crate mpsc channel for tcp connection
        let (mut sender, mut reciever ) = 
            tokio::sync::mpsc::channel(MAX_MSG_SIZE);
        

        let handle = tokio::task::spawn( async move{
            //bind to tcp port for any address
            let tcp_listener = TcpListener::bind(format!("0.0.0.0:{}", TCP_PORT)).await?;
            loop {
                //accept tcp connection
                let mut tcp_stream = tcp_listener.accept().await?.0;

                let mut buf = Vec::with_capacity(MAX_MSG_SIZE);
                while tcp_stream.read(&mut buf[..]).await? != 0 {
                    sender.send(& buf).await?;
                }
            }
        }).await??;

        Ok(reciever)
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

impl UserNode {
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




