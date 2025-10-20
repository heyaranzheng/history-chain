use std::net::SocketAddr;

use tokio::net::{UdpSocket};
use tokio::task::spawn;

use crate::constants::{
    MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, UDP_CHECK_PORT,
};
use crate::herrors::HError;
use crate::network::identity::Identity;
use crate::network::protocol::Message;
use crate::network::protocol::Header;

pub trait UdpConnection {
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
        &self, 
        dst_addr: SocketAddr, 
        msg: &Message,
        identity: & mut Identity,
    ) -> Result<usize, HError> 
    
    {
        //encode message into buffer with a header
        let mut uninit_buffer = Vec::with_capacity(MAX_UDP_MSG_SIZE);
        unsafe { uninit_buffer.set_len(MAX_UDP_MSG_SIZE) };
        let header_size = Header::header_size();
        let total_size = msg.encode_into_slice(identity, &mut uninit_buffer[..])? + header_size;

        //bind self address and udp port
        let my_addr = SocketAddr::from(
            format!("0.0.0.0:{}", UDP_CHECK_PORT)
        );
        let udp_socket =UdpSocket::bind( my_addr).await?;
        //send data to dst_addr 
        let _ = udp_socket.send_to(&uninit_buffer[..total_size], dst_addr).await?;
        Ok(total_size)
        
    }

    ///-------------------USE DIFFERENT PORTS FOR ADDRESS FILTERING-------------------
    ///send message to all addresses of specified node to find the avaiable address of node we 
    ///want to connect to. 
    /// 
    /// -----------------design a new way to a specified random check_port for a more accurate 
    /// checking process.
    async fn filter_addr_list(
        &self, 
        addr_list: Vec<SocketAddr>, 
        msg: &Message, 
        identity: & mut Identity,
    ) -> Result<(), HError> {

        if addr_list.is_empty() {
            return Err(HError::new("addr_list is empty"));
        }

        //create a recv task to receive message from check_port
        let mut buffer = [0u8; MAX_UDP_MSG_SIZE];
        let mut check_port_msg_list 
            = self.udp_recv_from_check_port(&addr_list);
        
      
        for addr in addr_list {
            let mut id = identity.clone();
            let check_addr = addr.clone().set_port(UDP_CHECK_PORT);
            spawn(self.udp_send_to( addr,msg,  id));
        }
        Ok(())
    
    }

    ///udp connection, recive message from CHECK_PORT.
    /// 
    ///-----------If we can negotiate a random unused CHECK_PORT, not a global one, that 
    /// will be more accurate.
    async fn udp_recv_from_check_port(
        &self, 
        addr_list: &Vec<SocketAddr>,
    ) -> Result<SocketAddr, HError>  {
        let addr = SocketAddr::from(
            format!("0.0.0.0:{}", UDP_CHECK_PORT)
        );
        //find the response from the check_port
        loop {
            let (msg, src_addr) = self.udp_recv_from(addr).await?;
            if addr_list.contains(&src_addr) {
                let src_addr = src_addr.parse::<SocketAddr>()?;
                return Ok(src_addr)
            }
        }
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
        let dst_addr = "127.0.0.1:8081".to_string();
        let send_task = async move {
            let src_addr = format!("127.0.0.1:8080").to_string();
            test.udp_send_to( dst_addr, &msg, &mut Identity::new()).await.unwrap();
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

}
