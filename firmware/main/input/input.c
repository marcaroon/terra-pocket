#include "input.h"
#include "input_logic.h"
#include "board_config.h"
#include "driver/gpio.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/queue.h"
#include "freertos/task.h"

static QueueHandle_t events;
_Static_assert(pdMS_TO_TICKS(BOARD_INPUT_SAMPLE_MS) > 0, "Input sampling requires a fast enough RTOS tick");
_Static_assert(BOARD_ENCODER_DIRECTION == 1 || BOARD_ENCODER_DIRECTION == -1, "Encoder direction must be +/-1");
static const gpio_num_t buttons[] = {
    BOARD_BUTTON_PREVIOUS, BOARD_BUTTON_PLAY, BOARD_BUTTON_NEXT, BOARD_ENCODER_PUSH,
};

static uint8_t read_encoder(void)
{
    return (gpio_get_level(BOARD_ENCODER_A) << 1) | gpio_get_level(BOARD_ENCODER_B);
}

static void input_task(void *arg)
{
    (void)arg;
    button_filter_t filters[4];
    encoder_filter_t encoder;
    uint32_t now = (uint32_t)(esp_timer_get_time() / 1000);
    for (unsigned i = 0; i < 4; ++i) {
        button_filter_init(&filters[i], gpio_get_level(buttons[i]) == 0, now);
    }
    encoder_filter_init(&encoder, read_encoder(), BOARD_ENCODER_HALF_STEP);
    TickType_t wake = xTaskGetTickCount();
    uint32_t dropped = 0, last_report = now;
    for (;;) {
        now = (uint32_t)(esp_timer_get_time() / 1000);
        for (unsigned i = 0; i < 4; ++i) {
            if (button_filter_sample(&filters[i], gpio_get_level(buttons[i]) == 0,
                                     now, BOARD_INPUT_DEBOUNCE_MS)) {
                const input_event_t event = (input_event_t)i;
                if (xQueueSend(events, &event, 0) != pdTRUE) ++dropped;
            }
        }
        const int step = encoder_filter_sample(&encoder, read_encoder()) * BOARD_ENCODER_DIRECTION;
        if (step != 0) {
            const input_event_t event = step > 0 ? INPUT_VOLUME_UP : INPUT_VOLUME_DOWN;
            if (xQueueSend(events, &event, 0) != pdTRUE) ++dropped;
        }
        if (dropped && (uint32_t)(now - last_report) >= 1000) {
            ESP_LOGW("INPUT", "Queue full: dropped %lu events", (unsigned long)dropped);
            dropped = 0;
            last_report = now;
        }
        // shortcut: 1 ms sampling for a hand-operated encoder; use PCNT if measured fast turns lose steps.
        xTaskDelayUntil(&wake, pdMS_TO_TICKS(BOARD_INPUT_SAMPLE_MS));
    }
}

esp_err_t input_init(void)
{
    if (events != NULL) return ESP_ERR_INVALID_STATE;
    const gpio_config_t pins = {
        .pin_bit_mask = (1ULL << BOARD_BUTTON_PREVIOUS) | (1ULL << BOARD_BUTTON_PLAY)
                     | (1ULL << BOARD_BUTTON_NEXT) | (1ULL << BOARD_ENCODER_PUSH)
                     | (1ULL << BOARD_ENCODER_A) | (1ULL << BOARD_ENCODER_B),
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    esp_err_t err = gpio_config(&pins);
    if (err != ESP_OK) return err;
    events = xQueueCreate(32, sizeof(input_event_t));
    if (events == NULL) return ESP_ERR_NO_MEM;
    if (xTaskCreate(input_task, "physical_input", 3072, NULL, 6, NULL) != pdPASS) {
        vQueueDelete(events);
        events = NULL;
        return ESP_ERR_NO_MEM;
    }
    ESP_LOGI("INPUT", "Ready: active-low buttons, debounce %d ms, encoder %s",
             BOARD_INPUT_DEBOUNCE_MS, BOARD_ENCODER_HALF_STEP ? "half-step" : "full-step");
    return ESP_OK;
}

bool input_receive(input_event_t *event, TickType_t wait)
{
    return events != NULL && event != NULL && xQueueReceive(events, event, wait) == pdTRUE;
}
