use std::{
    str::{self, FromStr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use clap::Parser;
use hyper::{
    client::conn,
    header::{HOST, ACCEPT},
    http::HeaderValue,
    Body,
};
use hyper::{http::Request, Uri};
use tokio::{
    macros::support::poll_fn,
    net::TcpStream,
    task::JoinSet,
    time::{sleep_until, Duration, Instant},
};

const STARTUP_DELAY_NS: u64 = 100_000_000;
const TEST_DURATION_NS: u64 = 10_000_000_000;

const NUM_CONNECTIONS: u64 = 500;
const REQUESTS_PER_SECOND: u64 = 20000;

const RESPONSE: &[u8] = r#"{"message":"Hello, World!"}"#.as_bytes();

const ACCEPT_ALL_HEADER: HeaderValue = HeaderValue::from_static("*/*");

#[derive(Parser)]
#[command(name = "load-generator")]
#[command(author = "Jasper Teunissen <git@jasper.teunissen.io>")]
#[command(version = "1.0")]
#[command(about = "Sends a constant load to a web server", long_about = None)]
struct Cli {
    target: String,
    #[arg(short, long, default_value_t=NUM_CONNECTIONS)]
    connections: u64,
    #[arg(short, long, default_value_t=REQUESTS_PER_SECOND)]
    requests_per_second: u64,
    #[arg(short, long, default_value_t=TEST_DURATION_NS)]
    test_duration: u64,
    #[arg(long, default_value_t=STARTUP_DELAY_NS)]
    startup_delay: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let cli = Cli::parse();

    // # Preprocess immutable variables for reuse
    // Convert passed target to URI
    let uri = Uri::from_str(&cli.target)?;
    // Create a full uri with hardcoded schema
    let uri = Uri::builder()
        .scheme("http")
        .authority(match uri.authority() {
            // If no host is included, throw error
            None => {
                eprintln!(
                    "URL is in wrong format, doesn't have an authority! Given url: {}",
                    cli.target
                );
                return Ok(());
            }
            // If host is included, but no port, use default port 80
            Some(authority) if authority.port().is_none() => {
                let mut auth = authority.to_string();
                auth.push_str(":80");
                auth
            }
            // Fully qualified host
            Some(authority) => authority.to_string(),
        })
        .path_and_query(match uri.path_and_query() {
            // Path is optional
            Some(p_and_q) => p_and_q.as_str(),
            None => "/",
        })
        .build()?;
    // Pre-write the authority to a HeaderValue
    // Authority is safe to unwrap, because above code creates it if it doesn't exist
    let host_header_value = HeaderValue::from_str(uri.authority().unwrap().as_str())?;

    // Create a set to store all request results
    let mut tasks = JoinSet::new();
    let time_between_requests = 1_000_000_000 / cli.requests_per_second;
    let tot_num_requests = cli.test_duration / time_between_requests;

    // This is the start time for the benchmark. Should be a little in the future to allow some time for creating the tasks.
    let start_time = Instant::now() + Duration::from_nanos(cli.startup_delay);

    // Create atomic test counter
    let test_counter = Arc::new(AtomicU64::new(0));

    // Create tasks for each parallel connection we want
    for _ in 0..cli.connections {
        // Required cloning for sending to tasks
        let test_counter = Arc::clone(&test_counter);
        let host_header_value = host_header_value.clone();
        let uri = uri.clone();

        tasks.spawn(connection_handler(
            uri,
            test_counter,
            host_header_value,
            tot_num_requests,
            start_time,
            time_between_requests,
        ));
    }

    while let Some(el) = tasks.join_next().await {
        match el {
            Ok(Ok(arr)) => arr.iter().for_each(|body| {
                eprintln!(
                    "Got unexpected response: {}",
                    str::from_utf8(body).unwrap_or("Binary data")
                )
            }),
            Ok(Err(Error::Hyper(e))) => eprintln!("Got a hyper error: {}", e),
            Ok(Err(Error::Http(e))) => eprintln!("Got a http error: {}", e),
            Ok(Err(Error::Std(e))) => eprintln!("Got an IO error: {}", e),
            Err(e) => eprintln!("Got a tokio error: {}", e),
        }
    }

    eprintln!("FINISHED TEST!");

    Ok(())
}

async fn connection_handler(
    uri: Uri,
    test_counter: Arc<AtomicU64>,
    host_header_value: HeaderValue,
    tot_num_requests: u64,
    start_time: Instant,
    time_between_requests: u64,
) -> Result<Vec<Vec<u8>>, Error> {
    // Create vec to collect wrong responses
    let mut wrong_responses = Vec::new();
    let outer_test_counter = Arc::clone(&test_counter);

    // Function to retrieve next deadline
    let get_deadline = &|| {
        let n = test_counter.fetch_add(1, Ordering::SeqCst);
        if n < tot_num_requests {
            Some(start_time + Duration::from_nanos(n * time_between_requests))
        } else {
            None
        }
    };

    while outer_test_counter.load(Ordering::SeqCst) < tot_num_requests {
        let result = single_connection_handler(
                uri.clone(),
                host_header_value.clone(),
                get_deadline,
            )
            .await;
        match result {
            Ok(mut new_wrong_responses) => wrong_responses.append(&mut new_wrong_responses),
            Err(Error::Hyper(e)) if e.is_closed() => continue,
            Err(e) => return Err(e),
        };
    }

    Ok(wrong_responses)
}

async fn single_connection_handler<F>(
    uri: Uri,
    host_header_value: HeaderValue,
    get_deadline: &F,
) -> Result<Vec<Vec<u8>>, Error> where F: Fn() -> Option<Instant> {
    // Open a connection
    let target_stream = TcpStream::connect(uri.authority().unwrap().as_str()).await?;
    let (mut request_sender, connection) = conn::handshake(target_stream).await?;

    // spawn a task to poll the connection and drive the HTTP state
    tokio::spawn(async move {
        let bla = connection.await;
        if let Err(e) = bla {
            eprintln!("Error in connection: {}", e);
        }
    });

    // Create vec to collect wrong responses
    let mut wrong_responses = Vec::new();

    // Loop while we can get new deadlines
    while let Some(deadline) = get_deadline() {
        // To send via the same connection again, it may not work as it may not be ready,
        // so we have to wait until the request_sender becomes ready.
        poll_fn(|cx| request_sender.poll_ready(cx)).await?;

        // When the connection is ready before the next deadline, wait longer
        if Instant::now() < deadline {
            sleep_until(deadline).await;
        }

        // let debug_uri = uri.clone();
        // eprintln!("{:?}\n{:?}{:?}{:?}{:?}{:?}\n{:?}", debug_uri, debug_uri.scheme_str(), debug_uri.host(), debug_uri.port_u16(), debug_uri.path(), debug_uri.query(), host_header_value.clone());
        // eprintln!("{:?}", debug_uri.into_parts());

        let request = Request::builder()
            // We need to manually add the host header because SendRequest does not
            .header(HOST, host_header_value.clone())
            .header(ACCEPT, ACCEPT_ALL_HEADER)
            .uri(uri.clone().path())
            .body(Body::empty())?;

        // eprintln!("STARTING REQUEST");

        let response = request_sender.send_request(request).await?;

        // eprintln!("STARTING TO FETCH BODY");

        // // Check required format. This is to filter out errors.
        let body = response.into_body();

        // eprintln!("GETTING BODY");
        let full_body = hyper::body::to_bytes(body).await?.to_vec();
        // eprintln!("FETCHED BODY");

        if full_body != RESPONSE {
            println!("WRONG RESPONSE!!");
            wrong_responses.push(full_body);
        }
    }

    Ok(wrong_responses)
}

#[derive(Debug)]
enum Error {
    Hyper(hyper::Error),
    Http(hyper::http::Error),
    Std(std::io::Error),
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Error::Hyper(err)
    }
}

impl From<hyper::http::Error> for Error {
    fn from(err: hyper::http::Error) -> Self {
        Error::Http(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Std(err)
    }
}
