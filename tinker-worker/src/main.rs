use tinker_worker::gen_img;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    // return the image as a png
    let start = std::time::Instant::now();
    let data = gen_img().await.unwrap();
    // write the image to a file
    tokio::fs::write("data/test.png", data).await.unwrap();
    println!("wrote image to data/test.png in {:?}", start.elapsed());

    let start = std::time::Instant::now();
    let data2 = gen_img().await.unwrap();
    tokio::fs::write("data/test2.png", data2).await.unwrap();
    println!("wrote image to data/test2.png in {:?}", start.elapsed());
}
