# tinker-arduino

Code for an ESP8266 that displays images from a Cloudflare worker on a
[Waveshare 7.5in 800x480 e-ink display](https://www.waveshare.com/7.5inch-e-paper-hat.htm).

## How does it work?

The ESP8266 uses the [GxEPD2](https://github.com/ZinggJM/GxEPD2) library
to communicate with the e-ink display.

Data for the image to display is fetched from the worker using HTTPS,
stored using 1 bit per pixel to save space. The ESP8266 then displays the
image on the e-ink display. The image is refreshed every 60 seconds.
