// https://mybusnow.njtransit.com/bustime/eta/getStopPredictionsETA.jsp?agency=&route=all&stop=31497
use chrono::TimeZone;
use serde::Deserialize;

pub const PARK_AVE_STOP: u32 = 31497;
pub const BLVD_EAST_STOP: u32 = 21824;

#[derive(Debug)]
pub struct StopArrival {
    pub route_number: i32,
    pub arrival_time: chrono::DateTime<chrono_tz::Tz>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct NJTStopArrival {
    #[serde(rename = "rid")]
    route_id: String,
    #[serde(rename = "tripid")]
    trip_id: String,
    #[serde(rename = "schdtm")]
    scheduled_time: String,
    #[serde(rename = "geoid")]
    geo_id: String,
    seq: u32,
    #[serde(rename = "tmstmp")]
    timestamp: String,
    typ: String,
    #[serde(rename = "stpnm")]
    stop_name: String,
    #[serde(rename = "stpid")]
    stop_id: String,
    #[serde(rename = "vid")]
    vehicle_id: String,
    dstp: u32,
    #[serde(rename = "rt")]
    route: String,
    rtdd: String,
    #[serde(rename = "rtdir")]
    route_dir: String,
    #[serde(rename = "des")]
    description: String,
    #[serde(rename = "prdtm")]
    predicted_time: String,
    tablockid: String,
    tatripid: String,
    origtatripno: String,
    dly: bool,
    #[serde(rename = "prdctdn")]
    predicted_n: String,
    zone: String,
}

pub async fn get_arrival_details(stop_id: u32) -> Vec<StopArrival> {
    let url = format!(
        "https://app.njtransit.com/NJTAppWS4/services/getMBNPredictions?stopid={}",
        stop_id
    );

    let client = reqwest::Client::builder()
        .user_agent("okhttp/4.10.0")
        .build()
        .unwrap();
    let resp = client
        .get(&url)
        .basic_auth("njtapp", Some("8rg3rX8G"))
        .send()
        .await
        .unwrap();
    let text = resp.text().await.unwrap();
    let text = text.strip_prefix("callback(").unwrap();
    let text = text.strip_suffix(")").unwrap();
    let arrivals: Vec<NJTStopArrival> = serde_json::from_str(text).unwrap_or(vec![]);
    // sleep for 1 second to avoid rate limiting
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    arrivals
        .into_iter()
        .map(|a| {
            let arrival_time =
                chrono::NaiveDateTime::parse_from_str(&a.predicted_time, "%Y%m%d %H:%M").unwrap();
            let arrival_time = chrono_tz::US::Eastern
                .from_local_datetime(&arrival_time)
                .unwrap();
            StopArrival {
                route_number: a.route.parse::<i32>().unwrap(),
                arrival_time: arrival_time,
            }
        })
        .collect()
}
