use tokio::net::{UdpSocket};

use crate::constants::{
    UDP_RECV_PORT, MAX_UDP_MSG_SIZE
};
use crate::herrors::HError;
use crate::message::Message;


///udp connection, receive message from all nodes, return message and source address
async fn udp_recv_from() -> Result< (Message, String), HError> {
    let mut buf = vec![0; MAX_UDP_MSG_SIZE];

    //bind to udp port for any address
    let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_RECV_PORT)).await?;
    //receive data from udp socket
    let (size, src_addr) = udp_socket.recv_from(&mut buf).await?;
    let src_addr_str = format!("{}:{}", src_addr.ip(), src_addr.port());
    
    //deserialize data to message
    let msg = Message::decode_from_slice(&buf[..size])?;

    Ok((msg, src_addr_str))
}
///udp connection, send message to another node
async fn udp_send_to(my_addr: String, dst_addr: String, msg: &Message) -> Result<usize, HError> 
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
    let udp_socket =UdpSocket::bind(format!("{}", my_addr)).await?;
    //send data to dst_addr
    let _ = udp_socket.send_to(&msg_encoded[..], dst_addr).await?;
    Ok(msg_encoded.len())
    
}