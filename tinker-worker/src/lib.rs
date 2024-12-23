use log::info;

use bitvec::vec::BitVec;
use tiny_skia::Pixmap;

use chrono::{TimeZone, Timelike};
use nj_transit::PARK_AVE_STOP;
use usvg::{fontdb, NodeKind, NormalizedF32, Tree, TreeParsing, TreeTextToPath};

use crate::nj_transit::{StopArrival, BLVD_EAST_STOP};

mod nj_transit;
mod pirate_weather;

const WEATHER_LAT: f64 = 40.774370;
const WEATHER_LONG: f64 = -74.019892;
// cache weather data for 30 minutes to limit API calls to < 5000 per month
const WEATHER_EXPIRY_SECS: i64 = 1800;

fn get_pirate_weather_api_key() -> String {
    std::env::var("PIRATE_WEATHER_API_KEY").expect("PIRATE_WEATHER_API_KEY not set")
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct WeatherForecast {
    temp: f64,
    high: f64,
    low: f64,
    hourly_precip: Vec<f64>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct CachedWeatherForecast {
    weather: WeatherForecast,
    expiry: i64,
    start_of_day: i64,
}

async fn fetch_weather<Tz: TimeZone>(
    now: chrono::DateTime<Tz>,
) -> Result<WeatherForecast, Box<dyn std::error::Error>> {
    // convert now to unix timestamp of the start of the day
    let sod = now
        .with_hour(0)
        .unwrap()
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .timestamp();

    // check for cached weather data in the bucket
    if let Ok(contents) = tokio::fs::read_to_string("data/weather.json").await {
        if let Ok(cached) = serde_json::from_str::<CachedWeatherForecast>(&contents) {
            // if unexpired, return the cached data
            if cached.expiry >= now.timestamp()
                && cached.start_of_day == sod
            {
                info!("weather cache hit");
                return Ok(cached.weather);
            }
        }
    }
    info!("weather cache miss");

    let mut weather = pirate_weather::fetch_pirate_weather(
        get_pirate_weather_api_key().as_str(),
        WEATHER_LAT,
        WEATHER_LONG,
        sod,
    )
    .await?;

    // update the actual current weather
    weather.currently = pirate_weather::fetch_pirate_weather(
        get_pirate_weather_api_key().as_str(),
        WEATHER_LAT,
        WEATHER_LONG,
        now.timestamp(),
    ).await?.currently;

    for (i, forecast) in weather.hourly.data.iter().enumerate() {
        let diff = forecast.time - sod;
        let hours = diff / 3600;
        assert!(hours == i as i64);
    }

    let hourly_temp = weather
        .hourly
        .data
        .iter()
        .map(|f| f.temperature)
        .collect::<Vec<_>>();

    let res = WeatherForecast {
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
    };

    // cache the weather data in the bucket for 1 hour
    let cached = CachedWeatherForecast {
        weather: res.clone(),
        expiry: now.timestamp() + WEATHER_EXPIRY_SECS,
        start_of_day: sod,
    };
    info!("caching weather data: {:?}", cached);
    let json_str = serde_json::to_string(&cached)?;
    tokio::fs::write("data/weather.json", json_str).await?;
    Ok(res)
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

async fn generate_tree(svg_data: String, opt: usvg::Options) -> Result<Tree, Box<dyn std::error::Error>> {
    info!("get now");
    let now = chrono::Utc::now().with_timezone(&chrono_tz::US::Eastern);
    info!("get weather");
    let forecast = fetch_weather(now).await?;
    info!("get arrivals");
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

    info!("get tree");
    let mut tree = Tree::from_str(&svg_data, &opt)?;
    info!("modify text");
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

async fn get_tree() -> Result<resvg::Tree, Box<dyn std::error::Error>> {
    info!("in get_tree");
    
    // Load and cache template SVG data
    static TEMPLATE_DATA: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    let svg_data = TEMPLATE_DATA.get_or_init(|| async {
        tokio::fs::read_to_string("data/template.svg").await.unwrap()
    }).await;

    // Load and cache font data
    static FONT_DATA: tokio::sync::OnceCell<fontdb::Database> = tokio::sync::OnceCell::const_new();
    let fontdb = FONT_DATA.get_or_init(|| async {
        let mut db = fontdb::Database::new();
        db.load_font_data(tokio::fs::read("data/fonts/BebasNeue-Regular.ttf").await.unwrap());
        db.load_font_data(tokio::fs::read("data/fonts/Louis George Cafe.ttf").await.unwrap());
        db.load_font_data(tokio::fs::read("data/fonts/Louis George Cafe Bold.ttf").await.unwrap());
        db
    }).await;

    let mut opt = usvg::Options::default();
    opt.shape_rendering = usvg::ShapeRendering::CrispEdges;
    
    info!("call generate_tree");
    let mut tree = generate_tree(svg_data.clone(), opt).await?;

    info!("convert text");
    tree.convert_text(fontdb);
    Ok(resvg::Tree::from_usvg(&tree))
}

pub async fn get_pixmap() -> Result<Pixmap, Box<dyn std::error::Error>> {
    let rtree = get_tree().await?;
    info!("got tree");
    let pixmap_size = rtree.size.to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    rtree.render(tiny_skia::Transform::default(), &mut pixmap.as_mut());
    info!("got pixmap");
    Ok(pixmap)
}

pub async fn gen_img() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // return the image as a png
    let pixmap = get_pixmap().await?;
    let data = pixmap.encode_png()?;
    info!("encoded png");
    Ok(data)
}

pub async fn gen_raw() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // return the image as raw bytes (1 byte per pixel)
    let pixmap = get_pixmap().await?;
    let data: BitVec<u8> = pixmap
        .pixels()
        .into_iter()
        .map(|pixel| {
            if pixel.red() != 0 || pixel.green() != 0 || pixel.blue() != 0 {
                true
            } else {
                false
            }
        })
        .collect();
    let (_, body, tail) = data.domain().region().unwrap();
    if tail.is_some() {
        panic!("pixmap size is not a multiple of 8");
    }
    info!("encoded raw");
    Ok(body.to_vec())
}
