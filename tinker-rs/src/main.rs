use std::path::Path;

use chrono::{TimeZone, Timelike};
use dotenv::dotenv;
use nj_transit::PARK_AVE_STOP;
use usvg::{fontdb, NodeKind, NormalizedF32, Tree, TreeParsing, TreeTextToPath};

use crate::nj_transit::{StopArrival, BLVD_EAST_STOP};

mod nj_transit;
mod pirate_weather;

struct WeatherForecast {
    temp: f64,
    high: f64,
    low: f64,
    hourly_precip: Vec<f64>,
}

async fn fetch_weather<Tz: TimeZone>(now: chrono::DateTime<Tz>) -> WeatherForecast {
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
        std::env::var("PIRATE_WEATHER_API_KEY")
            .expect("PIRATE_WEATHER_API_KEY must be set.")
            .as_str(),
        40.776085,
        -74.019334,
        now,
    )
    .await;

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

    WeatherForecast {
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
    }
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

async fn generate_tree<P: AsRef<Path>>(path: P, opt: usvg::Options) -> Tree {
    let now = chrono::Utc::now().with_timezone(&chrono_tz::US::Eastern);
    let forecast = fetch_weather(now).await;
    let blvd_east_arrivals = nj_transit::get_arrival_details(BLVD_EAST_STOP)
        .await
        .into_iter()
        .filter(|a| {
            a.route_number == 128
                || a.route_number == 165
                || a.route_number == 166
                || a.route_number == 168
        })
        .collect::<Vec<_>>();
    let park_ave_arrivals = nj_transit::get_arrival_details(PARK_AVE_STOP)
        .await
        .into_iter()
        .filter(|a| a.route_number == 156 || a.route_number == 89)
        .collect::<Vec<_>>();

    // get the percentage of the day that has passed
    let percent = now.time().num_seconds_from_midnight() as f32 / 86400.0;
    let svg_data = std::fs::read_to_string(path).unwrap().replace(
        "id=\"precip-time\" x=\"50%\"",
        format!("id=\"precip-time\" x=\"{}%\"", percent * 100.0).as_str(),
    );

    let mut tree = Tree::from_str(&svg_data, &opt).unwrap();
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

    tree
}

async fn get_tree() -> resvg::Tree {
    let mut opt = usvg::Options::default();
    opt.shape_rendering = usvg::ShapeRendering::CrispEdges;
    let mut tree = generate_tree("./template.svg", opt).await;
    let mut fontdb = fontdb::Database::new();
    fontdb.load_font_file("./BebasNeue-Regular.ttf").unwrap();
    fontdb.load_font_file("./Louis George Cafe.ttf").unwrap();
    fontdb
        .load_font_file("./Louis George Cafe Bold.ttf")
        .unwrap();
    tree.convert_text(&fontdb);
    resvg::Tree::from_usvg(&tree)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv().ok();
    let rtree = get_tree().await;
    let pixmap_size = rtree.size.to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    rtree.render(tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap.save_png("out.png").unwrap();
}
