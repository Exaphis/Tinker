use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ForecastData {
    pub time: i64,
    pub summary: String,
    #[serde(rename = "precipProbability")]
    pub precip_probability: f64,
    pub temperature: f64,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct HourlyForecast {
    pub summary: String,
    pub icon: String,
    pub data: Vec<ForecastData>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct PirateForecast {
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    pub offset: f64,
    pub elevation: f64,
    pub currently: ForecastData,
    pub hourly: HourlyForecast,
}

pub async fn fetch_pirate_weather(
    api_key: &str,
    lat: f64,
    long: f64,
    timestamp: i64,
) -> Result<PirateForecast, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.pirateweather.net/forecast/{}/{},{},{}?exclude=minutely,daily",
        api_key, lat, long, timestamp
    );
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    let forecast = response.json::<PirateForecast>().await?;
    Ok(forecast)
}
