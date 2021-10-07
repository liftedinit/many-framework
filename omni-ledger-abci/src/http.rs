use rocket::{get, launch, Build, Error, Rocket};

use std::net::ToSocketAddrs;

#[get("/hello/<name>/<age>")]
fn hello(name: &str, age: u8) -> String {
    format!("Hello, {} year old named {}!", age, name)
}

#[get("/")]
fn hello_world() -> String {
    "Hello World".into()
}

pub async fn launch<Host: ToSocketAddrs>(host: Host) -> Result<(), Error> {
    let addr = host.to_socket_addrs().unwrap().next().unwrap();
    let config = rocket::config::Config {
        address: addr.ip(),
        port: addr.port(),
        temp_dir: "/tmp/omni-ledger".into(),
        ..rocket::Config::debug_default()
    };

    rocket::custom(&config).launch().await
}
