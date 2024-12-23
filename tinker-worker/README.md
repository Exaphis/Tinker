# tinker-worker

Axum server that generates bitmap images to display on a 800x480 screen.

## How does it work?

Images are generated from a template SVG file using [resvg](https://github.com/RazrFalcon/resvg).

Weather data shown on the bitmap is gathered from [Pirate Weather](http://pirateweather.net/).
Bus arrival times are gathered from the NJ Transit API
(reverse engineered from the Android app).

## Setup

1. Set the `PIRATE_WEATHER_API_KEY` environment variable (can put in `.env`).
2. `cd <project_root> && cargo run`. Data will be loaded from `data/`.

## Usage

Run `cargo run` to start the server.

Run `cargo run --release` to start the server in release mode.

Use `RUST_LOG=tinker_worker=info` to see info logs.
