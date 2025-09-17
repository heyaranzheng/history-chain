use tokio::net::TcpStream;
use std::net::SocketAddr;

///signals used to  communicate with other threads
pub enum Signal {
    Close,
    ListenResult(TcpStream, String),
    
}
impl Signal {
    ///create a new listen result signal
    pub fn from_accept_result( listen_result: (TcpStream, SocketAddr)) -> Self {
        let sockaddr_str = format!("{}:{}", listen_result.1.ip(), listen_result.1.port());
        Self::ListenResult(listen_result.0, sockaddr_str)
    }
}