use tinker_worker::gen_img;
use log::info;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let app = axum::Router::new()
        .route("/img", axum::routing::get(img_handler))
        .route("/raw", axum::routing::get(raw_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
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
