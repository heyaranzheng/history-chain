use tokio::net::{UdpSocket};

use crate::constants::{
    MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, UDP_RECV_PORT
};
use crate::herrors::HError;
use crate::network::identity::Identity;
use crate::network::protocol::Message;
use crate::network::protocol::Header;

pub trait UdpConnection {
    ///udp connection, receive message from all nodes, return message and source address
    async fn udp_recv_from(&mut self) -> Result< (Message, String), HError> {
        let mut uninit_buffer = Vec::with_capacity(MAX_UDP_MSG_SIZE);
        unsafe { uninit_buffer.set_len(MAX_UDP_MSG_SIZE) };

        //bind to udp port for any address
        let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_RECV_PORT)).await?;
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
        &mut self, my_addr: String, 
        dst_addr: String, 
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
        let udp_socket =UdpSocket::bind(format!("{}", my_addr)).await?;
        //send data to dst_addr
        let _ = udp_socket.send_to(&uninit_buffer[..total_size], dst_addr).await?;
        Ok(total_size)
        
    }
}


mod tests {
    use super::*;


    #[tokio::test(flavor = "multi_thread")]
    async fn test_network() {
        let msg = Message::new_with_zero();
        let save_msg = msg.clone();
        struct Test;
        
        impl UdpConnection for Test {}
        let mut test = Test;
        let dst_addr = "127.0.0.1:8081".to_string();
        let send_task = async move {
            let src_addr = format!("127.0.0.1:8080").to_string();
            test.udp_send_to(src_addr, dst_addr, &msg, &mut Identity::new()).await.unwrap();
        };

        let mut test = Test;
        
        //crate a mpsc to send and recv message
        use tokio::sync::mpsc;
        let (sender, mut receiver) = mpsc::channel(1);
        let recv_task = async move {
            let (msg, src_addr) = test.udp_recv_from().await.unwrap();
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
