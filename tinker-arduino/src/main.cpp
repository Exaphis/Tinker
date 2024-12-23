#include <ESP8266WiFi.h>
#include <ESP8266HTTPClient.h>
#include <WiFiClientSecureBearSSL.h>
#include <GxEPD2_BW.h>

#include "secrets.h"

// #define ENABLE_PRINT
#ifndef ENABLE_PRINT
// disable Serial output
#define Serial PlaceholderName
static class {
   public:
    template <typename... T>
    void begin(T...) {}
    template <typename... T>
    void print(T...) {}
    template <typename... T>
    void printf(T...) {}
    template <typename... T>
    void println(T...) {}
} Serial;
#endif

// No need for paged display, set the page height to 1 to avoid extra RAM usage
GxEPD2_BW<GxEPD2_750_T7, 1> display(GxEPD2_750_T7(/*CS=D8*/ SS, /*DC=D3*/ 0, /*RST=D4*/ 2, /*BUSY=D2*/ 4)); // GDEW075T7 800x480, EK79655 (GD7965)
const size_t BUF_LEN = GxEPD2_750_T7::WIDTH / 8;
uint8_t row_buf[BUF_LEN];

const unsigned long REFRESH_INTERVAL_MS = 180UL * 1000UL;
const unsigned int WIFI_TIMEOUT_SEC = 30;
// max number of refreshes before clearing screen
const unsigned int MAX_REFRESH_COUNT = 5;  

uint8_t refresh_count = 0;

void show_raw_bitmap(void);

void setup() {
    Serial.begin(115200);
    Serial.println();
    Serial.println("Tinker-Arduino started.");

    // blink builtin LED 3 times to indicate boot
    pinMode(LED_BUILTIN, OUTPUT);
    for (int i = 0; i < 3; i++) {
        digitalWrite(LED_BUILTIN, LOW);
        delay(1000);
        digitalWrite(LED_BUILTIN, HIGH);
        delay(1000);
    }

    Serial.print("Connecting to ");
    Serial.print(WIFI_SSID);

    WiFi.mode(WIFI_STA);
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

    int start_time = millis();
    while (WiFi.status() != WL_CONNECTED) {
        Serial.printf(".(%d)", WiFi.status());
        if (millis() - start_time > WIFI_TIMEOUT_SEC * 1000) {
            Serial.println();
            Serial.println("Connection timed out.");
            ESP.restart();
        }
        delay(1000);
    }

    Serial.println();
    Serial.println("WiFi connected.");
    Serial.print("IP address: ");
    Serial.println(WiFi.localIP());
    
    display.init(115200);
}

void loop() {
    if (WiFi.status() != WL_CONNECTED) {
        ESP.restart();
        return;
    }

    if (refresh_count == 0) {
        Serial.println("Clearing display...");
        display.clearScreen();
        display.refresh();
        Serial.println("Display cleared");
    }

    Serial.println("displaying image...");
    show_raw_bitmap();
    Serial.printf("done one display, refresh count=%d\n", refresh_count);

    // Power off display to avoid burn-in
    // See https://www.waveshare.com/wiki/7.5inch_e-Paper_HAT_Manual#Precautions
    // Cannot use display.hibernate() because it will lose previous image,
    // preventing fast partial refresh
    display.powerOff();
    delay(REFRESH_INTERVAL_MS);
}

uint8_t reverse_bits(uint8_t v) {
    // http://graphics.stanford.edu/~seander/bithacks.html#BitReverseObvious
    uint8_t r = v; // r will be reversed bits of v; first get LSB of v
    int s = sizeof(v) * CHAR_BIT - 1; // extra shift needed at end

    for (v >>= 1; v; v >>= 1) {   
        r <<= 1;
        r |= v & 1;
        s--;
    }
    r <<= s; // shift when v's highest bits are zero
    return r;
}

bool display_row(HTTPClient &http, std::unique_ptr<BearSSL::WiFiClientSecure> &client, int row) {
    if (!http.connected()) {
        Serial.println("Error: HTTP connection not open");
        return false;
    }

    int c = client->readBytes(row_buf, BUF_LEN);
    if (c < static_cast<int>(BUF_LEN)) {
        Serial.printf("Error: only read %d bytes, expected %d\n", c, BUF_LEN);
        return false;
    }

    // Not sure why but the bits need to be reversed
    for (size_t i = 0; i < BUF_LEN; i++) {
        row_buf[i] = reverse_bits(row_buf[i]);
    }

    Serial.printf("Writing row %d\n", row);
    display.writeImage(row_buf, 0, row, display.width(), 1);
    return true;
}

void show_raw_bitmap() {
    std::unique_ptr<BearSSL::WiFiClientSecure> client(new BearSSL::WiFiClientSecure);
    client->setInsecure();
    client->setBufferSizes(4096, 4096);
    client->setTimeout(10000);  // 10 seconds timeout due to slow image generation

    HTTPClient http;
    Serial.println("Connecting to host...");
    if (!http.begin(*client, "tinker.kev3u.com", 443, "/raw")) {
        Serial.println("Connection failed");
        return;
    }

    int code = http.GET();
    if (code != HTTP_CODE_OK) {
        Serial.printf("HTTP error: %d\n", code);
        return;
    }

    Serial.println("Reading image data...");
    for (int row = 0; row < display.height(); row++) {
        if (!display_row(http, client, row)) {
            break;
        }
    }
    Serial.println("Done reading image data");
    // Do a fast refresh to avoid flickering when the screen is updated
    display.refresh(true);
    refresh_count++;
    refresh_count %= MAX_REFRESH_COUNT;
}