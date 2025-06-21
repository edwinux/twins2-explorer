// TWINS-Core/twins_node_rust/src/p2p/peer.rs
// This file will define the Peer struct and related peer management logic.

use tokio::net::TcpStream;
use std::net::SocketAddr;
use crate::p2p::messages::VersionMessage; // To store peer's version info

#[derive(Debug)]
pub struct Peer {
    pub address: SocketAddr,
    // The stream is optional because it will be taken by the dedicated session handler task.
    pub stream: Option<TcpStream>,
    pub version_info: Option<VersionMessage>, // Store received version message
    pub connected: bool,
    pub handshake_completed: bool,
    // Fields to store extracted version information
    pub user_agent: Option<String>,
    pub start_height: Option<i32>,
    pub services: Option<u64>,
}

impl Peer {
    pub fn new(address: SocketAddr, stream: TcpStream) -> Self {
        Peer {
            address,
            stream: Some(stream), // Stream is present upon creation
            version_info: None,
            connected: true, 
            handshake_completed: false,
            user_agent: None,
            start_height: None,
            services: None,
        }
    }

    // Method to take the stream, consuming the Option
    pub fn take_stream(&mut self) -> Option<TcpStream> {
        self.stream.take()
    }

    pub fn set_version_data(&mut self, version_info: &VersionMessage) {
        self.user_agent = Some(version_info.user_agent.clone());
        self.start_height = Some(version_info.start_height);
        self.services = Some(version_info.services);
        self.version_info = Some(version_info.clone()); // Store the whole message if needed
    }
} 