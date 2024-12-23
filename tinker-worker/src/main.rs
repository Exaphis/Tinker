use tinker_worker::gen_img;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    // return the image as a png
    let data = gen_img().await.unwrap();
    // write the image to a file
    tokio::fs::write("data/test.png", data).await.unwrap();
}
