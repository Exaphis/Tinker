use std::io::Cursor;

use image::{DynamicImage, ImageFormat};
use worker::{wasm_bindgen::UnwrapThrowExt, *};

use chrono::{TimeZone, Timelike};
use nj_transit::PARK_AVE_STOP;
use usvg::{fontdb, NodeKind, NormalizedF32, Tree, TreeParsing, TreeTextToPath};

use crate::nj_transit::{StopArrival, BLVD_EAST_STOP};

mod nj_transit;
mod pirate_weather;

const PIRATE_WEATHER_API_KEY: &str = "PIRATE_WEATHER_API_KEY";
const TINKER_BUCKET: &str = "TINKER_BUCKET";

struct WeatherForecast {
    temp: f64,
    high: f64,
    low: f64,
    hourly_precip: Vec<f64>,
}

async fn fetch_weather<Tz: TimeZone>(
    now: chrono::DateTime<Tz>,
    env: Env,
) -> Result<WeatherForecast> {
    // convert now to unix timestamp of the start of the day
    let now = now
        .with_hour(0)
        .unwrap()
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap();
    let now = now.timestamp();

    let weather = pirate_weather::fetch_pirate_weather(
        env.secret(PIRATE_WEATHER_API_KEY)?.to_string().as_str(),
        40.774370,
        -74.019892,
        now,
    )
    .await?;

    for (i, forecast) in weather.hourly.data.iter().enumerate() {
        let diff = forecast.time - now;
        let hours = diff / 3600;
        assert!(hours == i as i64);
    }

    let hourly_temp = weather
        .hourly
        .data
        .iter()
        .map(|f| f.temperature)
        .collect::<Vec<_>>();

    Ok(WeatherForecast {
        temp: weather.currently.temperature,
        high: hourly_temp
            .iter()
            .map(|f| *f)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap(),
        low: hourly_temp
            .iter()
            .map(|f| *f)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap(),
        hourly_precip: weather
            .hourly
            .data
            .iter()
            .map(|f| f.precip_probability)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
    })
}

trait GetText {
    fn modify_node_text(&mut self, node_id: &str, f: impl Fn(&mut usvg::Text) -> ()) -> ();
}

impl GetText for Tree {
    fn modify_node_text(&mut self, node_id: &str, f: impl Fn(&mut usvg::Text) -> ()) {
        let node = self.node_by_id(node_id);
        if node.is_none() {
            panic!("{} not found", node_id);
        }
        let node = node.unwrap();
        let mut node = node.borrow_mut();
        if let NodeKind::Text(ref mut text) = *node {
            f(text);
        } else {
            panic!("{} is not a text node", node_id);
        }
    }
}

async fn generate_tree(svg_data: String, opt: usvg::Options, env: Env) -> Result<Tree> {
    console_log!("get now");
    let now = chrono::Utc::now().with_timezone(&chrono_tz::US::Eastern);
    console_log!("get weather");
    let forecast = fetch_weather(now, env).await?;
    console_log!("get arrivals");
    let blvd_east_arrivals = nj_transit::get_arrival_details(BLVD_EAST_STOP)
        .await?
        .into_iter()
        .filter(|a| {
            a.route_number == 128
                || a.route_number == 165
                || a.route_number == 166
                || a.route_number == 168
        })
        .collect::<Vec<_>>();
    let park_ave_arrivals = nj_transit::get_arrival_details(PARK_AVE_STOP)
        .await?
        .into_iter()
        .filter(|a| a.route_number == 156 || a.route_number == 89)
        .collect::<Vec<_>>();

    // get the percentage of the day that has passed
    let percent = now.time().num_seconds_from_midnight() as f32 / 86400.0;
    let svg_data = svg_data.replace(
        "id=\"precip-time\" x=\"50%\"",
        format!("id=\"precip-time\" x=\"{}%\"", percent * 100.0).as_str(),
    );

    console_log!("get tree");
    let mut tree = Tree::from_str(&svg_data, &opt).unwrap();
    console_log!("modify text");
    tree.modify_node_text("text-time", |time| {
        // Format time as 12-hour clock with AM/PM
        time.chunks[0].text = now.format("%-I:%M %p").to_string().to_lowercase();
    });

    tree.modify_node_text("text-date", |date| {
        // Format date as "Monday, Jul 12"
        date.chunks[0].text = now.format("%A").to_string();
        date.chunks[1].text = now.format("%b %-d").to_string();
    });

    tree.modify_node_text("text-curr-temp", |temp| {
        temp.chunks[0].text = format!("{:.0}", forecast.temp);
    });

    tree.modify_node_text("text-hi-lo-temp", |temp| {
        temp.chunks[0].text = format!("{:.0}", forecast.high);
        temp.chunks[1].text = format!("{:.0}", forecast.low);
    });

    let node = tree.node_by_id("precip-lines").unwrap();
    for (i, child) in node.children().enumerate() {
        let mut child = child.borrow_mut();
        if let NodeKind::Group(ref mut group) = *child {
            group.transform.sy = forecast.hourly_precip[i] as f32;
        } else {
            panic!("precip-lines child is not a group node");
        }
    }

    fn set_stop_arrivals(tree: &mut Tree, name: &str, arrivals: Vec<StopArrival>) {
        const MAX_ARRIVALS: usize = 4;
        for i in (arrivals.len() + 1)..=MAX_ARRIVALS {
            let node_id = format!("{}-{}", name, i);
            let node = tree.node_by_id(node_id.as_str()).unwrap();
            let mut node = node.borrow_mut();
            if let NodeKind::Group(ref mut group) = *node {
                group.opacity = NormalizedF32::ZERO;
            } else {
                panic!("{} not a group node", node_id);
            }
        }

        for (i, arrival) in arrivals.into_iter().take(MAX_ARRIVALS).enumerate() {
            tree.modify_node_text(format!("{}-{}-route", name, i + 1).as_str(), |temp| {
                temp.chunks[0].text = format!("{}", arrival.route_number);
            });
            tree.modify_node_text(format!("{}-{}-time", name, i + 1).as_str(), |temp| {
                temp.chunks[0].text = arrival.arrival_time.format("%-I:%M %p").to_string();
            });
        }
    }

    set_stop_arrivals(&mut tree, "park", park_ave_arrivals);
    set_stop_arrivals(&mut tree, "blvd", blvd_east_arrivals);

    Ok(tree)
}

async fn get_tree(env: Env) -> Result<resvg::Tree> {
    console_log!("in get_tree");
    let mut opt = usvg::Options::default();
    opt.shape_rendering = usvg::ShapeRendering::CrispEdges;
    console_log!("call generate_tree");
    let bucket = env.bucket(TINKER_BUCKET)?;
    console_log!("bucket found, getting template data");
    let svg_data = bucket
        .get("template.svg")
        .execute()
        .await?
        .expect_throw("svg object not found")
        .body()
        .expect_throw("svg body not found")
        .text()
        .await?;
    let mut tree = generate_tree(svg_data, opt, env).await?;

    async fn get_font_data(path: &str, bucket: &Bucket) -> Result<Vec<u8>> {
        bucket
            .get(path)
            .execute()
            .await?
            .expect_throw("font object not found")
            .body()
            .expect_throw("font body not found")
            .bytes()
            .await
    }

    console_log!("gen fonts");
    let mut fontdb = fontdb::Database::new();
    fontdb.load_font_data(get_font_data("fonts/BebasNeue-Regular.ttf", &bucket).await?);
    fontdb.load_font_data(get_font_data("fonts/Louis George Cafe.ttf", &bucket).await?);
    fontdb.load_font_data(get_font_data("fonts/Louis George Cafe Bold.ttf", &bucket).await?);
    tree.convert_text(&fontdb);
    Ok(resvg::Tree::from_usvg(&tree))
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if !matches!(req.method(), Method::Get) {
        return Response::error("Method Not Allowed", 405);
    }
    console_log!("{}: in main", req.path());
    if req.path() != "/" {
        return Response::error("Not Found", 404);
    }

    let rtree = get_tree(env).await?;
    console_log!("got tree");
    let pixmap_size = rtree.size.to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())
        .expect_throw("failed to create pixmap");
    rtree.render(tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // convert pixmap to bmp for output to arduino
    let img_buf = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
        pixmap_size.width() as u32,
        pixmap_size.height() as u32,
        pixmap.data().to_vec(),
    )
    .expect_throw("failed to create image buffer");
    let mut buffer = Cursor::new(vec![]);
    let img_buf = DynamicImage::ImageRgba8(img_buf).into_luma8();
    img_buf
        .write_to(&mut buffer, ImageFormat::Bmp)
        .expect_throw("failed to write bmp");

    let mut headers = Headers::new();
    headers.set("Content-Type", "image/bmp")?;
    Ok(Response::from_body(ResponseBody::Body(buffer.into_inner()))?.with_headers(headers))
}
