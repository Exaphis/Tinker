use tinker_worker::gen_img;
use log::info;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let matches = clap::Command::new("tinker-worker")
        .about("Start the Tinker display server.")
        .arg(clap::Arg::new("host")
            .short('h')
            .long("host")
            .value_name("HOST")
            .help("Host address to bind to")
            .default_value("0.0.0.0"))
        .arg(clap::Arg::new("port")
            .short('p')
            .long("port")
            .value_name("PORT")
            .help("Port to listen on")
            .default_value("3000"))
        .get_matches();

    let host = matches.get_one::<String>("host").unwrap();
    let port = matches.get_one::<String>("port").unwrap();
    let addr = format!("{}:{}", host, port);

    let app = axum::Router::new()
        .route("/img", axum::routing::get(img_handler))
        .route("/raw", axum::routing::get(raw_handler));

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

async fn img_handler() -> axum::response::Response {
    info!("img_handler");
    match gen_img().await {
        Ok(data) => axum::response::Response::builder()
            .header("Content-Type", "image/png")
            .body(axum::body::Body::from(data))
            .unwrap(),
        Err(e) => {
            eprintln!("Error generating image: {}", e);
            axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::from("Internal Server Error"))
                .unwrap()
        }
    }
}

async fn raw_handler() -> axum::response::Response {
    info!("raw_handler");
    match tinker_worker::gen_raw().await {
        Ok(data) => axum::response::Response::builder()
            .header("Content-Type", "application/octet-stream") 
            .body(axum::body::Body::from(data))
            .unwrap(),
        Err(e) => {
            eprintln!("Error generating raw data: {}", e);
            axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::from("Internal Server Error"))
                .unwrap()
        }
    }
}
