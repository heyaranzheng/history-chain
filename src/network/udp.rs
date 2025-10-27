use std::net::{Ipv4Addr, IpAddr, SocketAddr};
use async_trait::async_trait;
use futures::future::join_all;

use tokio::net::{UdpSocket};
use tokio::task::spawn;
use tokio::time::timeout;

use crate::constants::{
    MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, UDP_CHECK_PORT,
};
use crate::herrors::HError;
use crate::nodes::Identity;
use crate::network::protocol::{Message, Payload, Header};
use crate::hash::HashValue;

#[async_trait]
pub trait UdpConnection: Send + Sync {
    ///udp connection, receive message from all nodes, return message and source address
    ///
    async fn udp_recv_from(&self, bind_addr: SocketAddr) -> Result< (Message, String), HError> {
        let mut uninit_buffer = Vec::with_capacity(MAX_UDP_MSG_SIZE);
        unsafe { uninit_buffer.set_len(MAX_UDP_MSG_SIZE) };

        //bind to udp port for any address
        let udp_socket = UdpSocket::bind(bind_addr).await?;
        //receive data from udp socket
        let (size, src_addr) = udp_socket.recv_from(&mut uninit_buffer).await?;
        let src_addr_str = format!("{}:{}", src_addr.ip(), src_addr.port());
        
        //get header from the buffer
        let header_size = Header::header_size();
        let header = Header::decode_from_slice(&uninit_buffer[..header_size])?;

        //decode message from buffer with header
        let msg = 
            Message::decode_from_slice(&uninit_buffer[header_size..size], & header)?;

        Ok((msg, src_addr_str))
    }

    ///udp connection, send message to another node
    async fn udp_send_to(
        dst_addr: SocketAddr, 
        msg: &Message,
        identity: & mut Identity,
    ) -> Result<usize, HError> 
        where Self: Send + Sync
    
    {
        //encode message into buffer with a header
        let mut uninit_buffer = Vec::with_capacity(MAX_UDP_MSG_SIZE);
        unsafe { uninit_buffer.set_len(MAX_UDP_MSG_SIZE) };
        let header_size = Header::header_size();
        let total_size = msg.encode_into_slice(identity, &mut uninit_buffer[..])? + header_size;

        //bind self address and udp port
        let my_addr = 
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), UDP_CHECK_PORT);
        let udp_socket =UdpSocket::bind( my_addr).await?;
        //send data to dst_addr 
        let _ = udp_socket.send_to(&uninit_buffer[..total_size], dst_addr).await?;
        Ok(total_size)
        
    }

    ///an helper function for check_addresses_available, check if a addr is available
    async fn check_addr_available(
        addr: &SocketAddr, 
        timeout_duration: std::time::Duration,
        msg_byte: &[u8],
    ) -> Result<SocketAddr, HError> {
        //create a socket and send an message to dst_addr
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(udp_socket) => {
                let result = timeout(timeout_duration, 
                    //send data to dst_addr 
                    udp_socket.send_to(msg_byte, addr)
                ).await;

                match result {
                    Ok(_) => Ok(*addr),
                    Err(_) => Err(
                        HError::NetWork { 
                            message: format!("check_addr_available:{} timeout", addr)
                        }
                    )
                }
            },
    
            Err(_) => { 
                Err(
                    HError::NetWork { 
                        message: format!("check_addr_available:udp socket bind error for sending") 
                    }
                )
            }
        }
        
    }
   
    async fn check_addresses_available(
        addr_list: &Vec<SocketAddr>, 
        timeout_ms: u64,
        receiver: HashValue,
        identity: & mut Identity,
    ) -> Result< Vec<SocketAddr>, HError> 
        where Self: Send + Sync
    {
        //check if the addr_list is empty.
        if addr_list.is_empty() {
            return Err(HError::NetWork { message: format!("upd_connection error: have no addr_list") });
        }

        //a buffer for encoded message
        let mut buffer = vec![0u8; MAX_MSG_SIZE];

        //time out duration, 1 second.
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        //create a message and sign it with identity, encode it into a byte array.
        let test_msg = Message::new(identity.public_key_to_bytes(), receiver, Payload::Empty);
        let msg_len = test_msg.encode_into_slice(identity, &mut buffer[..])?;

        //create tasks to check if each address is available 
        let tasks = addr_list.iter().map(|addr| {
            let buffer_clone = buffer.clone();
            async move {
                Self::check_addr_available(addr, timeout_duration, &buffer_clone[..msg_len]).await
            }
        });

        //wait for all tasks to complete and collect the available addresses.
        let results = join_all(tasks).await;
        let addresses_available = results.iter().filter_map(
            |result| {
                match result {
                    Ok(addr) => {
                        Some(*addr)
                    },
                    Err(_) => {
                        None
                    }
                }
            }
        ).collect();

        Ok(addresses_available)
    }
    

   
}


mod tests {
    use super::*;
    use crate::constants::ZERO_HASH;
    use crate::network::protocol::Payload;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_network() {
        let msg = Message::new(
            ZERO_HASH, ZERO_HASH, Payload::Empty);
        let save_msg = msg.clone();
        struct Test;
        
        impl UdpConnection for Test {}
        let mut test = Test;
        let dst_addr = SocketAddr::new(
            std::net::Ipv4Addr::new(127, 0, 0, 1).into(), 
            8081
        );
        let send_task = async move {
            let src_addr = format!("127.0.0.1:8080").to_string();
            Test::udp_send_to( dst_addr, &msg, &mut Identity::new()).await.unwrap();
        };

        let mut test = Test;
        
        //crate a mpsc to send and recv message
        use tokio::sync::mpsc;
        let (sender, mut receiver) = mpsc::channel(1);
        let recv_task = async move {
            let (msg, src_addr) = test.udp_recv_from("127.0.0.1:8080".parse().unwrap()).await.unwrap();
            sender.send( (msg, src_addr)).await.unwrap();
        };
        //if we sync the code below, we should use recv_task first, then send_task, 
        tokio::spawn(send_task);
        tokio::spawn(recv_task);
        let (recv_msg, src_addr) = receiver.recv().await.unwrap();
        assert_eq!(save_msg, recv_msg);
        assert_eq!(src_addr, "127.0.0.1:8080"); 
         
    }

    fn test_filter_addr_list() {

    }

}
