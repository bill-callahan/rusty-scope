// src/mount/alpacadiscovery.rs
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::str;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use log::info;

#[derive(Serialize)]
struct AlpacaDiscoveryResponse {
    #[serde(rename = "AlpacaPort")]
    alpaca_port: u16,
    
}

pub fn create_alpaca_discovery(ip: Ipv4Addr) {
    // Bind to UDP port 32227 on all interfaces
    let socket = match UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 32227)) {
        Ok(sock) => {
            info!("Bound to 0.0.0.0:32227");
            sock
        }
        Err(e) => {
            log::error!("Failed to bind socket: {:?}", e);
            return;
        }
    };

    // Allow broadcasting
    if let Err(e) = socket.set_broadcast(true) {
        log::error!("Failed to set broadcast: {:?}", e);
        return;
    }

    //log::warn!("Skipping multicast group join (not required for broadcast discovery)");
    // // Join the multicast group 224.0.0.251
    // unsafe {
    // // Import C types
    // let sock_fd = socket.as_raw_fd();

    // // Build ip_mreq manually
    // let mut mreq: libc::ip_mreq = std::mem::zeroed();

    // // Set multicast group: 224.0.0.251
    // mreq.imr_multiaddr.s_addr = u32::from_be_bytes([224, 0, 0, 251]);

    // // Set interface to INADDR_ANY (0.0.0.0)
    // mreq.imr_interface.s_addr = u32::from_be_bytes([0, 0, 0, 0]);

    // if libc::setsockopt(
    //     sock_fd,
    //     libc::IPPROTO_IP,
    //     libc::IP_ADD_MEMBERSHIP,
    //     &mreq as *const _ as *const _,
    //     std::mem::size_of_val(&mreq) as u32,
    // ) < 0 {
    //     log::error!("Failed to join multicast group");
    // } else {
    //     info!("Joined multicast group 224.0.0.251");
    // }
    // }
    info!("Listening for Alpaca discovery requests..{}:32227", ip);

    let mut buf = [0; 1024];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, src)) => {
                let msg = match str::from_utf8(&buf[..size]) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                info!("Received discovery request: {}", msg);

                if msg.contains("alpaca") || msg == "alpacadiscovery1" {
                    info!("Discovery request from {}", src);

                    let response = AlpacaDiscoveryResponse {
                        alpaca_port: 3000,
                        
                    };

                    let json_response = match serde_json::to_string(&response) {
                        Ok(json) => json,
                        Err(e) => {
                            log::error!("Failed to serialize JSON: {:?}", e);
                            continue;
                        }
                    };

                    match socket.send_to(json_response.as_bytes(), src) {
                        Ok(_) => info!("Sent discovery response to {}", src),
                        Err(e) => log::error!("Failed to send response: {:?}", e),
                    }
                }
            }
            Err(e) => {
                log::error!("Error receiving packet: {:?}", e);
            }
        }

        // Yield to avoid busy looping
        thread::sleep(Duration::from_millis(100));
    }
}