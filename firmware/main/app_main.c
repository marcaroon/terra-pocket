#include "app_state.h"
#include "board_config.h"
#include "diag/pin_scan.h"
#include "display/display.h"
#include "esp_log.h"
#include "esp_psram.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "input/input.h"
#include "storage/usb_probe.h"
#include "sdkconfig.h"

void app_main(void)
{
    ESP_LOGI("APP", "Terra - M2 input diagnostics, LCD 240x240");
    ESP_LOGI("APP", "PSRAM detected: %u bytes", (unsigned)esp_psram_get_size());
    if (esp_psram_get_size() != BOARD_EXPECTED_PSRAM) {
        ESP_LOGW("APP", "Expected N16R8 PSRAM size; verify board variant");
    }
    if (BOARD_DIAG_PIN_SCAN) pin_scan_run();
    ESP_ERROR_CHECK(app_state_init());
    ESP_ERROR_CHECK(display_init());
    app_state_load_mock();
    media_state_t snapshot;
    app_state_snapshot(&snapshot);
    ESP_ERROR_CHECK(display_render(&snapshot));
    ESP_LOGI("APP", "Mock screen ready");
    ESP_LOGI("APP", "Main task stack free: %u bytes",
             (unsigned)uxTaskGetStackHighWaterMark(NULL));
    ESP_ERROR_CHECK(input_init());
#if CONFIG_TERRA_USB_PROBE
    usb_probe_start();
#else
    ESP_LOGI("APP", "USB HDD probe disabled (normal input build)");
#endif
    ESP_LOGI("APP", "Input events are diagnostic only; Windows/BLE not connected");
    input_event_t event;
    for (;;) {
        if (!input_receive(&event, portMAX_DELAY)) continue;
        switch (event) {
        case INPUT_PREVIOUS: ESP_LOGI("INPUT", "PREVIOUS"); break;
        case INPUT_PLAY_PAUSE: ESP_LOGI("INPUT", "PLAY_PAUSE"); break;
        case INPUT_NEXT: ESP_LOGI("INPUT", "NEXT"); break;
        case INPUT_MUTE: ESP_LOGI("INPUT", "MUTE"); break;
        case INPUT_VOLUME_UP: ESP_LOGI("INPUT", "VOLUME +%d", BOARD_VOLUME_STEP); break;
        case INPUT_VOLUME_DOWN: ESP_LOGI("INPUT", "VOLUME -%d", BOARD_VOLUME_STEP); break;
        }
    }
}
