// use std::net::ToSocketAddrs;
//
// pub async fn launch<Host: ToSocketAddrs>(host: Host) -> Result<(), Error> {
//     let addr = host.to_socket_addrs().unwrap().next().ok_or()?;
//     let config = rocket::config::Config {
//         address: addr.ip(),
//         port: addr.port(),
//         temp_dir: "/tmp/omni-ledger".into(),
//         ..rocket::Config::debug_default()
//     };
//
//     rocket::custom(&config).launch().await
// }
