use std::time::Duration;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(long)]
    pub listen: String,

    #[arg(long)]
    pub upstream: String,

    #[arg(long, default_value_t = 0)]
    pub client_to_server_latency: u64,

    #[arg(long, default_value_t = 0)]
    pub client_to_server_jitter: u64,

    #[arg(long)]
    pub client_to_server_bandwidth: Option<u64>,

    #[arg(long)]
    pub server_to_client_bandwidth: Option<u64>,

    #[arg(long, requires = "client_to_server_bandwidth")]
    pub client_to_server_burst: Option<u64>,

    #[arg(long, requires = "server_to_client_bandwidth")]
    pub server_to_client_burst: Option<u64>,

    #[arg(long, default_value_t = 0)]
    pub server_to_client_latency: u64,

    #[arg(long, default_value_t = 0)]
    pub server_to_client_jitter: u64,

    #[arg(long)]
    pub hex: bool,
}

impl Clone for Args {
    fn clone(&self) -> Self {
        Self {
            listen: self.listen.clone(),
            upstream: self.upstream.clone(),
            client_to_server_latency: self.client_to_server_latency,
            client_to_server_jitter: self.client_to_server_jitter,
            client_to_server_bandwidth: self.client_to_server_bandwidth,
            server_to_client_bandwidth: self.server_to_client_bandwidth,
            client_to_server_burst: self.client_to_server_burst,
            server_to_client_burst: self.server_to_client_burst,
            server_to_client_latency: self.server_to_client_latency,
            server_to_client_jitter: self.server_to_client_jitter,
            hex: self.hex,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DirectionConfig {
    pub latency: Duration,
    pub jitter: Duration,

    pub bandwidth: Option<u64>, // bytes/second
    pub burst: Option<u64>,
}

#[derive(Debug)]
pub struct Config {
    pub upstream: String,
    pub client_to_server: DirectionConfig,
    pub server_to_client: DirectionConfig,
    pub hex: bool,
}

impl From<Args> for Config {
    fn from(args: Args) -> Self {
        Self {
            upstream: args.upstream,
            client_to_server: DirectionConfig {
                latency: Duration::from_millis(args.client_to_server_latency),
                jitter: Duration::from_millis(args.client_to_server_jitter),
                bandwidth: args.client_to_server_bandwidth,
                burst: args.client_to_server_burst,
            },
            server_to_client: DirectionConfig {
                latency: Duration::from_millis(args.server_to_client_latency),
                jitter: Duration::from_millis(args.server_to_client_jitter),
                bandwidth: args.server_to_client_bandwidth,
                burst: args.server_to_client_burst,
            },
            hex: args.hex,
        }
    }
}
