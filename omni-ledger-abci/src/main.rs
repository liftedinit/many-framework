pub mod application;
pub mod http;

use clap::Clap;
use tendermint_abci::ServerBuilder;
use tracing_subscriber::filter::LevelFilter;

#[derive(Debug, Clap)]
struct Opt {
    /// Bind the TCP server to this host.
    #[clap(long, default_value = "127.0.0.1")]
    abci_host: String,

    /// Bind the TCP server to this port.
    #[clap(long, default_value = "26658")]
    abci_port: u16,

    /// The default server read buffer size, in bytes, for each incoming client
    /// connection.
    #[clap(short, long, default_value = "1048576")]
    read_buf_size: usize,

    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long)]
    verbose: bool,

    /// Suppress all output logging (overrides --verbose).
    #[clap(short, long)]
    quiet: bool,

    // OMNI Protocol Host interface to listen to.
    #[clap(long, default_value = "127.0.0.1")]
    omni_host: String,

    // OMNI Protocol Port interface to listen to.
    #[clap(long, default_value = "8000")]
    omni_port: u16,
}

fn main() {
    let opt: Opt = Opt::parse();
    let log_level = if opt.quiet {
        LevelFilter::OFF
    } else if opt.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();

    let (app, driver) = application::KeyValueStoreApp::new();
    let abci_server = ServerBuilder::new(opt.read_buf_size)
        .bind(format!("{}:{}", opt.abci_host, opt.abci_port), app)
        .unwrap();

    let j1 = std::thread::spawn(move || driver.run().unwrap());
    // let j2 = std::thread::spawn(move || {
    //     let rt = tokio::runtime::Runtime::new().unwrap();
    //     rt.block_on(http::launch((opt.omni_host, opt.omni_port)))
    //         .unwrap();
    // });
    let j3 = std::thread::spawn(move || abci_server.listen().unwrap());

    print!("1");
    j1.join().unwrap();
    // print!("2");
    // j2.join().unwrap();
    print!("3");
    j3.join().unwrap();
}
