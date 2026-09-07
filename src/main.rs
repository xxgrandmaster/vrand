mod buffer;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::Mutex,
    time::{interval, sleep},
};

use buffer::{RollingBuffer, BUFFER_SIZE};

const WIKIMEDIA_URL: &str =
    "https://stream.wikimedia.org/v2/stream/recentchange";

const RIPE_URL: &str =
    "https://ris-live.ripe.net/v1/stream/?format=json&client=vrand";

const VRAND_VERSION: &str = "VRAND-0.1";

#[derive(Parser)]
#[command(
    name = "vrand",
    version,
    about = "VRAND — Verifiable Randomness from live public data"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show the live VRAND dashboard.
    Live,

    /// Create one VRAND hash from fresh live source data.
    CreateHash,

    /// Create a VRAND-derived random integer in an inclusive range.
    RandomInt {
        /// Minimum possible value.
        #[arg(long)]
        min: u64,

        /// Maximum possible value.
        #[arg(long)]
        max: u64,
    },
}

#[derive(Clone)]
struct SourceState {
    name: &'static str,
    connected: bool,
    bytes_received: u64,
    messages_received: u64,
    hash: [u8; 32],
    buffer_len: usize,
}

impl SourceState {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            connected: false,
            bytes_received: 0,
            messages_received: 0,
            hash: [0u8; 32],
            buffer_len: 0,
        }
    }

    fn ready(&self) -> bool {
        self.buffer_len == BUFFER_SIZE
    }
}

struct AppState {
    wikimedia: SourceState,
    ripe: SourceState,
}

impl AppState {
    fn new() -> Self {
        Self {
            wikimedia: SourceState::new("WIKIMEDIA"),
            ripe: SourceState::new("RIPE RIS"),
        }
    }

    fn ready(&self) -> bool {
        self.wikimedia.ready() && self.ripe.ready()
    }

    /// H3 = SHA-256(H1 || H2)
    fn final_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();

        hasher.update(self.wikimedia.hash);
        hasher.update(self.ripe.hash);

        hasher.finalize().into()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Live => run_live().await?,
        Commands::CreateHash => create_hash().await?,
        Commands::RandomInt { min, max } => random_int(min, max).await?,
    }

    Ok(())
}

async fn create_client() -> Result<Client> {
    Client::builder()
        .user_agent("VRAND/0.1.0")
        .build()
        .context("Failed to create HTTP client")
}

fn spawn_sources(
    client: &Client,
    state: &Arc<Mutex<AppState>>,
) {
    let wiki_state = Arc::clone(state);
    let wiki_client = client.clone();

    tokio::spawn(async move {
        if let Err(error) = run_wikimedia(wiki_client, wiki_state).await {
            eprintln!("Wikimedia task stopped: {error:#}");
        }
    });

    let ripe_state = Arc::clone(state);
    let ripe_client = client.clone();

    tokio::spawn(async move {
        if let Err(error) = run_ripe(ripe_client, ripe_state).await {
            eprintln!("RIPE RIS task stopped: {error:#}");
        }
    });
}

async fn run_live() -> Result<()> {
    enable_alternate_screen();

    let client = create_client().await?;
    let state = Arc::new(Mutex::new(AppState::new()));

    spawn_sources(&client, &state);

    draw_dashboard(state).await;

    disable_alternate_screen();

    Ok(())
}

async fn create_hash() -> Result<()> {
    println!();
    println!("VRAND CREATE HASH");
    println!("=================");
    println!();
    println!("Starting Wikimedia and RIPE RIS streams...");
    println!("Waiting for both 256 KiB buffers to fill.");
    println!();

    let client = create_client().await?;
    let state = Arc::new(Mutex::new(AppState::new()));

    spawn_sources(&client, &state);

    enable_cursor_hide();

    loop {
        {
            let app = state.lock().await;

            print!("\r\x1B[2K");

            print!(
                "Wikimedia: {:>6}/{:<6} bytes | RIPE RIS: {:>6}/{:<6} bytes",
                app.wikimedia.buffer_len,
                BUFFER_SIZE,
                app.ripe.buffer_len,
                BUFFER_SIZE
            );

            io::stdout().flush().ok();

            if app.ready() {
                break;
            }
        }

        sleep(Duration::from_millis(250)).await;
    }

    enable_cursor_show();

    let app = state.lock().await;

    let final_hash = app.final_hash();
    let creation_time = Utc::now();

    println!();
    println!();
    println!("==============================================================");
    println!("                     VRAND HASH CREATED");
    println!("==============================================================");
    println!();

    println!("Protocol        : {}", VRAND_VERSION);
    println!("Creation time   : {}", creation_time.to_rfc3339());
    println!();

    println!("Wikimedia bytes : {}", app.wikimedia.buffer_len);
    println!(
        "Wikimedia SHA256: {}",
        hex::encode(app.wikimedia.hash)
    );
    println!();

    println!("RIPE bytes      : {}", app.ripe.buffer_len);
    println!(
        "RIPE SHA256     : {}",
        hex::encode(app.ripe.hash)
    );
    println!();

    println!("Derivation:");
    println!("  H1 = SHA256(Wikimedia buffer)");
    println!("  H2 = SHA256(RIPE RIS buffer)");
    println!("  H3 = SHA256(H1 || H2)");
    println!();

    println!("VRAND HASH      : {}", hex::encode(final_hash));
    println!();

    Ok(())
}

async fn random_int(min: u64, max: u64) -> Result<()> {
    if min > max {
        anyhow::bail!("--min cannot be greater than --max");
    }

    println!();
    println!("VRAND RANDOM INT");
    println!("================");
    println!();
    println!("Requested range: {} - {}", min, max);
    println!("Collecting fresh live source data...");
    println!();

    let client = create_client().await?;
    let state = Arc::new(Mutex::new(AppState::new()));

    spawn_sources(&client, &state);

    enable_cursor_hide();

    loop {
        {
            let app = state.lock().await;

            print!("\r\x1B[2K");

            print!(
                "Wikimedia: {:>6}/{:<6} bytes | RIPE RIS: {:>6}/{:<6} bytes",
                app.wikimedia.buffer_len,
                BUFFER_SIZE,
                app.ripe.buffer_len,
                BUFFER_SIZE
            );

            io::stdout().flush().ok();

            if app.ready() {
                break;
            }
        }

        sleep(Duration::from_millis(250)).await;
    }

    enable_cursor_show();

    let app = state.lock().await;

    let final_hash = app.final_hash();
    let result = unbiased_integer(final_hash, min, max);
    let creation_time = Utc::now();

    println!();
    println!();
    println!("==============================================================");
    println!("                     VRAND RANDOM RESULT");
    println!("==============================================================");
    println!();

    println!("Protocol        : {}", VRAND_VERSION);
    println!("Creation time   : {}", creation_time.to_rfc3339());
    println!("Range           : {} - {}", min, max);
    println!();

    println!("VRAND HASH      : {}", hex::encode(final_hash));
    println!();
    println!("RANDOM INTEGER  : {}", result);
    println!();

    Ok(())
}

/// Maps the 256-bit VRAND output to [min, max] using rejection sampling.
fn unbiased_integer(hash: [u8; 32], min: u64, max: u64) -> u64 {
    let range = max - min + 1;

    if range == 1 {
        return min;
    }

    let mut candidate_hash = hash;

    loop {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&candidate_hash[..8]);

        let value = u64::from_be_bytes(bytes);

        let remainder = u64::MAX % range;
        let limit = u64::MAX - remainder;

        if value < limit {
            return min + (value % range);
        }

        let mut hasher = Sha256::new();
        hasher.update(b"VRAND-0.1-RANDOM-INT-RETRY");
        hasher.update(candidate_hash);

        candidate_hash = hasher.finalize().into();
    }
}

async fn run_wikimedia(
    client: Client,
    state: Arc<Mutex<AppState>>,
) -> Result<()> {
    loop {
        {
            let mut app = state.lock().await;
            app.wikimedia.connected = false;
        }

        match client.get(WIKIMEDIA_URL).send().await {
            Ok(response) => {
                {
                    let mut app = state.lock().await;
                    app.wikimedia.connected = response.status().is_success();
                }

                if !response.status().is_success() {
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }

                let mut stream = response.bytes_stream();
                let mut line_buffer = Vec::<u8>::new();
                let mut rolling = RollingBuffer::new(BUFFER_SIZE);

                while let Some(chunk_result) = stream.next().await {
                    let chunk = match chunk_result {
                        Ok(bytes) => bytes,
                        Err(_) => break,
                    };

                    line_buffer.extend_from_slice(&chunk);

                    while let Some(position) =
                        line_buffer.iter().position(|b| *b == b'\n')
                    {
                        let mut line =
                            line_buffer.drain(..=position).collect::<Vec<_>>();

                        if line.last() == Some(&b'\n') {
                            line.pop();
                        }

                        if line.last() == Some(&b'\r') {
                            line.pop();
                        }

                        // Wikimedia EventStreams uses SSE:
                        // data: {JSON}
                        if line.starts_with(b"data:") {
                            let payload = &line[5..];

                            let payload = if payload.starts_with(&[b' ']) {
                                &payload[1..]
                            } else {
                                payload
                            };

                            if payload.is_empty() {
                                continue;
                            }

                            // Confirm the payload is valid UTF-8.
                            if std::str::from_utf8(payload).is_err() {
                                continue;
                            }

                            rolling.push(payload);

                            let hash = rolling.sha256();

                            let mut app = state.lock().await;

                            app.wikimedia.bytes_received += payload.len() as u64;
                            app.wikimedia.messages_received += 1;
                            app.wikimedia.buffer_len = rolling.len();
                            app.wikimedia.hash = hash;
                            app.wikimedia.connected = true;
                        }
                    }
                }
            }

            Err(_) => {
                let mut app = state.lock().await;
                app.wikimedia.connected = false;
            }
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn run_ripe(
    client: Client,
    state: Arc<Mutex<AppState>>,
) -> Result<()> {
    loop {
        {
            let mut app = state.lock().await;
            app.ripe.connected = false;
        }

        match client.get(RIPE_URL).send().await {
            Ok(response) => {
                {
                    let mut app = state.lock().await;
                    app.ripe.connected = response.status().is_success();
                }

                if !response.status().is_success() {
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }

                let mut stream = response.bytes_stream();
                let mut line_buffer = Vec::<u8>::new();
                let mut rolling = RollingBuffer::new(BUFFER_SIZE);

                while let Some(chunk_result) = stream.next().await {
                    let chunk = match chunk_result {
                        Ok(bytes) => bytes,
                        Err(_) => break,
                    };

                    line_buffer.extend_from_slice(&chunk);

                    while let Some(position) =
                        line_buffer.iter().position(|b| *b == b'\n')
                    {
                        let mut line =
                            line_buffer.drain(..=position).collect::<Vec<_>>();

                        if line.last() == Some(&b'\n') {
                            line.pop();
                        }

                        if line.last() == Some(&b'\r') {
                            line.pop();
                        }

                        if line.is_empty() {
                            continue;
                        }

                        if std::str::from_utf8(&line).is_err() {
                            continue;
                        }

                        rolling.push(&line);

                        let hash = rolling.sha256();

                        let mut app = state.lock().await;

                        app.ripe.bytes_received += line.len() as u64;
                        app.ripe.messages_received += 1;
                        app.ripe.buffer_len = rolling.len();
                        app.ripe.hash = hash;
                        app.ripe.connected = true;
                    }
                }
            }

            Err(_) => {
                let mut app = state.lock().await;
                app.ripe.connected = false;
            }
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn draw_dashboard(state: Arc<Mutex<AppState>>) {
    let mut ticker = interval(Duration::from_millis(500));

    loop {
        ticker.tick().await;

        let app = state.lock().await;

        // Move cursor to top-left and clear the entire terminal.
        print!("\x1B[H\x1B[2J");

        let now = Utc::now();

        println!("==============================================================");
        println!("                     VRAND LIVE 0.1");
        println!("==============================================================");
        println!();
        println!("UTC TIME          : {}", now.to_rfc3339());
        println!();

        print_source(&app.wikimedia);

        println!();

        print_source(&app.ripe);

        println!();
        println!("--------------------------------------------------------------");
        println!("                         DERIVATION");
        println!("--------------------------------------------------------------");
        println!();

        println!(
            "H1  WIKIMEDIA     : {}",
            hex::encode(app.wikimedia.hash)
        );

        println!(
            "H2  RIPE RIS      : {}",
            hex::encode(app.ripe.hash)
        );

        println!();
        println!("H3 = SHA256(H1 || H2)");
        println!();

        println!(
            "VRAND OUTPUT      : {}",
            hex::encode(app.final_hash())
        );

        println!();
        println!(
            "Buffer size       : {} bytes (256 KiB)",
            BUFFER_SIZE
        );

        println!();
        println!("Press Ctrl+C to exit.");

        // Clear any leftover terminal content.
        print!("\x1B[J");

        io::stdout().flush().ok();
    }
}

fn print_source(source: &SourceState) {
    let status = if source.connected {
        "● CONNECTED"
    } else {
        "○ DISCONNECTED"
    };

    let percentage =
        (source.buffer_len as f64 / BUFFER_SIZE as f64) * 100.0;

    println!("{}", source.name);
    println!("  Status           : {}", status);
    println!("  Messages         : {}", source.messages_received);
    println!("  Bytes received   : {}", source.bytes_received);
    println!(
        "  Buffer           : {} / {} bytes ({:.2}%)",
        source.buffer_len,
        BUFFER_SIZE,
        percentage
    );
    println!("  SHA-256          : {}", hex::encode(source.hash));
}

fn enable_alternate_screen() {
    print!("\x1B[?1049h");
    print!("\x1B[?25l");
    io::stdout().flush().ok();
}

fn disable_alternate_screen() {
    print!("\x1B[?25h");
    print!("\x1B[?1049l");
    io::stdout().flush().ok();
}

fn enable_cursor_hide() {
    print!("\x1B[?25l");
    io::stdout().flush().ok();
}

fn enable_cursor_show() {
    print!("\x1B[?25h");
    io::stdout().flush().ok();
}
