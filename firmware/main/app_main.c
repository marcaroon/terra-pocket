#include "app_state.h"
#include "ble/ble_transport.h"
#include "board_config.h"
#include "diag/pin_scan.h"
#include "display/display.h"
#include "esp_app_desc.h"
#include "esp_log.h"
#include "esp_psram.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "input/input.h"
#include "nvs.h"
#include "nvs_flash.h"
#include "storage/usb_probe.h"
#include "sdkconfig.h"

/* The BLE controller keeps PHY calibration data in NVS. The partition holds no
 * user data, so an incompatible layout is erased and re-initialized. */
static void init_nvs(void)
{
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_LOGW("APP", "NVS layout changed; erasing NVS partition");
        ESP_ERROR_CHECK(nvs_flash_erase());
        err = nvs_flash_init();
    }
    ESP_ERROR_CHECK(err);
}

#define SETTINGS_NAMESPACE "terra"
#define SETTINGS_VIEW      "view"

static view_t load_view(void)
{
    nvs_handle_t handle;
    uint8_t value = VIEW_TEXT;
    if (nvs_open(SETTINGS_NAMESPACE, NVS_READONLY, &handle) == ESP_OK) {
        nvs_get_u8(handle, SETTINGS_VIEW, &value);
        nvs_close(handle);
    }
    return value == VIEW_ARTWORK ? VIEW_ARTWORK : VIEW_TEXT;
}

static void save_view(view_t view)
{
    nvs_handle_t handle;
    esp_err_t err = nvs_open(SETTINGS_NAMESPACE, NVS_READWRITE, &handle);
    if (err == ESP_OK) {
        err = nvs_set_u8(handle, SETTINGS_VIEW, (uint8_t)view);
        if (err == ESP_OK) err = nvs_commit(handle);
        nvs_close(handle);
    }
    if (err != ESP_OK) ESP_LOGW("APP", "View not saved: %s", esp_err_to_name(err));
}

void app_main(void)
{
    ESP_LOGI("APP", "Terra %s - media, volume and full-screen artwork (V2), LCD 240x240", esp_app_get_description()->version);
    ESP_LOGI("APP", "PSRAM detected: %u bytes", (unsigned)esp_psram_get_size());
    if (esp_psram_get_size() != BOARD_EXPECTED_PSRAM) {
        ESP_LOGW("APP", "Expected N16R8 PSRAM size; verify board variant");
    }
    if (BOARD_DIAG_PIN_SCAN) pin_scan_run();
    init_nvs();
    ESP_ERROR_CHECK(app_state_init());
    view_t view = load_view();
    app_state_set_view(view);
    ESP_ERROR_CHECK(display_init());
    ESP_ERROR_CHECK(display_start_ui());
    ESP_LOGI("APP", "Screen ready");
    ESP_LOGI("APP", "Main task stack free: %u bytes",
             (unsigned)uxTaskGetStackHighWaterMark(NULL));
    ESP_ERROR_CHECK(input_init());
    ESP_ERROR_CHECK(ble_transport_start());
#if CONFIG_TERRA_USB_PROBE
    usb_probe_start();
#else
    ESP_LOGI("APP", "USB HDD probe disabled (normal build)");
#endif
    ESP_LOGI("APP", "Media buttons and encoder volume/mute active when READY; long press switches the view");
    input_event_t event;
    for (;;) {
        if (!input_receive(&event, portMAX_DELAY)) continue;
        if (event == INPUT_VIEW_TOGGLE) {
            /* Local only: works without a computer and never reaches Windows. */
            view = view == VIEW_ARTWORK ? VIEW_TEXT : VIEW_ARTWORK;
            app_state_set_view(view);
            save_view(view);
            ESP_LOGI("INPUT", "VIEW %s", view == VIEW_ARTWORK ? "artwork" : "text");
            continue;
        }
        protocol_command_t command = {0};
        switch (event) {
        case INPUT_PREVIOUS: command.command = PROTOCOL_CMD_PREVIOUS; break;
        case INPUT_PLAY_PAUSE: command.command = PROTOCOL_CMD_PLAY_PAUSE; break;
        case INPUT_NEXT: command.command = PROTOCOL_CMD_NEXT; break;
        case INPUT_MUTE: command.command = PROTOCOL_CMD_MUTE_TOGGLE; break;
        case INPUT_VOLUME_UP:
            command = (protocol_command_t){PROTOCOL_CMD_VOLUME_DELTA, BOARD_VOLUME_STEP};
            break;
        case INPUT_VOLUME_DOWN:
            command = (protocol_command_t){PROTOCOL_CMD_VOLUME_DELTA, -BOARD_VOLUME_STEP};
            break;
        case INPUT_VIEW_TOGGLE:
            break;
        }
        /* The screen changes only when the companion reports the resulting state. */
        const esp_err_t err = ble_transport_send_command(&command);
        ESP_LOGI("INPUT", "MEDIA_COMMAND %u %d: %s", command.command, command.value, esp_err_to_name(err));
    }
}
