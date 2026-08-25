use clap::Parser;
use ip_discovery::{Config, IpVersion, Protocol, Strategy};
use std::process::ExitCode;
use std::time::Duration;

/// Discover your public IP address via DNS, STUN, or HTTP
#[derive(Parser)]
#[command(name = "ipd", version, about, long_about = None)]
struct Cli {
    /// Use IPv4 only
    #[arg(short = '4', long)]
    ipv4: bool,

    /// Use IPv6 only
    #[arg(short = '6', long)]
    ipv6: bool,

    /// Output format: plain, json, verbose
    #[arg(short, long, default_value = "plain")]
    format: OutputFormat,

    /// Discovery strategy: first, race, consensus
    #[arg(short, long, default_value = "first")]
    strategy: StrategyArg,

    /// Protocol filter: dns, stun, http (can be repeated)
    #[arg(short, long)]
    protocol: Vec<ProtocolArg>,

    /// Timeout per provider in seconds
    #[arg(short, long, default_value = "10")]
    timeout: u64,

    /// Show local private IP address instead of public IP
    #[arg(short = 'l', long, alias = "local")]
    private: bool,
}

#[derive(Clone, clap::ValueEnum)]
enum OutputFormat {
    Plain,
    Json,
    Verbose,
}

#[derive(Clone, clap::ValueEnum)]
enum StrategyArg {
    First,
    Race,
    Consensus,
}

#[derive(Clone, clap::ValueEnum)]
enum ProtocolArg {
    Dns,
    Stun,
    Http,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.private {
        let opt_ip = match (cli.ipv4, cli.ipv6) {
            (false, true) => ip_discovery::get_private_ipv6(),
            _ => ip_discovery::get_private_ip().or_else(ip_discovery::get_private_ipv6),
        };

        match opt_ip {
            Some(ip) => {
                match cli.format {
                    OutputFormat::Plain => {
                        println!("{}", ip);
                    }
                    OutputFormat::Json => {
                        let json = serde_json::json!({
                            "ip": ip.to_string(),
                            "type": if ip.is_ipv4() { "IPv4" } else { "IPv6" },
                            "scope": "private",
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json).unwrap_or_default()
                        );
                    }
                    OutputFormat::Verbose => {
                        println!("{}", ip);
                        println!("  scope: private");
                        println!("  type:  {}", if ip.is_ipv4() { "IPv4" } else { "IPv6" });
                    }
                }
                return ExitCode::SUCCESS;
            }
            None => {
                eprintln!(
                    "error: no local network interface found with a valid private IP address"
                );
                return ExitCode::FAILURE;
            }
        }
    }

    let version = match (cli.ipv4, cli.ipv6) {
        (true, false) => IpVersion::V4,
        (false, true) => IpVersion::V6,
        _ => IpVersion::Any,
    };

    let strategy = match cli.strategy {
        StrategyArg::First => Strategy::First,
        StrategyArg::Race => Strategy::Race,
        StrategyArg::Consensus => Strategy::Consensus { min_agree: 2 },
    };

    let mut builder = Config::builder()
        .version(version)
        .strategy(strategy)
        .timeout(Duration::from_secs(cli.timeout));

    if !cli.protocol.is_empty() {
        let protocols: Vec<Protocol> = cli
            .protocol
            .iter()
            .map(|p| match p {
                ProtocolArg::Dns => Protocol::Dns,
                ProtocolArg::Stun => Protocol::Stun,
                ProtocolArg::Http => Protocol::Http,
            })
            .collect();
        builder = builder.protocols(&protocols);
    }

    let config = builder.build();

    match ip_discovery::blocking::get_ip_with(config) {
        Ok(result) => {
            match cli.format {
                OutputFormat::Plain => {
                    println!("{}", result.ip);
                }
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "ip": result.ip.to_string(),
                        "provider": result.provider,
                        "protocol": format!("{}", result.protocol),
                        "latency_ms": result.latency.as_millis(),
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json).unwrap_or_default()
                    );
                }
                OutputFormat::Verbose => {
                    println!("{}", result.ip);
                    println!("  provider: {}", result.provider);
                    println!("  protocol: {}", result.protocol);
                    println!("  latency:  {}ms", result.latency.as_millis());
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
