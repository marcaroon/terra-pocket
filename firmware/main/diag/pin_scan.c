#include "pin_scan.h"

#include "driver/gpio.h"
#include "esp_log.h"
#include "esp_rom_sys.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "PIN_SCAN";

/* Header GPIOs that are safe to read as inputs after boot. Excludes flash/Octal
 * PSRAM (26-37), native USB (19/20) and the UART console (43/44). */
static const gpio_num_t pins[] = {
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
    21, 38, 39, 40, 41, 42, 45, 46, 47, 48,
};
#define PIN_COUNT (sizeof(pins) / sizeof(pins[0]))

static int read_with_pull(gpio_num_t pin, gpio_pull_mode_t pull)
{
    gpio_set_pull_mode(pin, pull);
    esp_rom_delay_us(2000);
    return gpio_get_level(pin);
}

void pin_scan_run(void)
{
    for (size_t i = 0; i < PIN_COUNT; i++) {
        gpio_reset_pin(pins[i]);
        gpio_set_direction(pins[i], GPIO_MODE_INPUT);
    }
    ESP_LOGI(TAG, "Static scan, do not touch any control:");
    for (size_t i = 0; i < PIN_COUNT; i++) {
        const int up = read_with_pull(pins[i], GPIO_PULLUP_ONLY);
        const int down = read_with_pull(pins[i], GPIO_PULLDOWN_ONLY);
        const char *state = up && !down ? "open (high-Z)"
            : !up && !down ? "TIED LOW (GND)"
            : up && down ? "TIED HIGH (3V3/pull-up)" : "inconsistent";
        ESP_LOGI(TAG, "GPIO%-2d %s", pins[i], state);
    }

    int level[PIN_COUNT];
    for (size_t i = 0; i < PIN_COUNT; i++) {
        level[i] = read_with_pull(pins[i], GPIO_PULLUP_ONLY);
    }
    ESP_LOGI(TAG, "Watching with pull-ups; press each control now");
    for (;;) {
        for (size_t i = 0; i < PIN_COUNT; i++) {
            const int now = gpio_get_level(pins[i]);
            if (now != level[i]) {
                ESP_LOGI(TAG, "GPIO%d %d->%d", pins[i], level[i], now);
                level[i] = now;
            }
        }
        vTaskDelay(pdMS_TO_TICKS(2));
    }
}
