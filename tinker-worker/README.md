# tinker-worker

Cloudflare worker that generates bitmap images to display on a 800x480 screen.

## How does it work?

Images are generated from a template SVG file using [resvg](https://github.com/RazrFalcon/resvg).

Weather data shown on the bitmap is gathered from [Pirate Weather](http://pirateweather.net/).
Bus arrival times are gathered from the NJ Transit API
(reverse engineered from the Android app).

## Setup

1. Update your `account_id` in `wrangler.toml`.
2. `npx wrangler r2 bucket create <name>`
   * Create two Cloudflare R2 buckets for production/development.
3. Update `wrangler.toml` with the bucket names.
4. Upload `template.svg` and the fonts in `fonts/` to the buckets.
5. Add `PIRATE_WEATHER_API_KEY=<key>` to `.dev.vars`.
   * Set the API key for the Pirate Weather API in development.
6. `npx wrangler secret put PIRATE_WEATHER_API_KEY`
   * Set the API key for the Pirate Weather API in production.

## Usage

Run `npx wrangler dev --remote` to run the development server.
Remote is needed to access the files stored in the R2 bucket.

Run `npx wrangler deploy` to publish the worker to production.
