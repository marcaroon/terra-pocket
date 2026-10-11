#include "app_state.h"
#include <assert.h>
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"

static SemaphoreHandle_t state_mutex;
static media_state_t state;

esp_err_t app_state_init(void)
{
    if (state_mutex != NULL) {
        return ESP_ERR_INVALID_STATE;
    }
    state_mutex = xSemaphoreCreateMutex();
    if (state_mutex == NULL) {
        return ESP_ERR_NO_MEM;
    }
    state = (media_state_t){.connection = CONNECTION_BOOT};
    return ESP_OK;
}

void app_state_load_mock(void)
{
    assert(state_mutex != NULL);
    xSemaphoreTake(state_mutex, portMAX_DELAY);
    state = (media_state_t){
        .connection = CONNECTION_DISCONNECTED,
        .demo = true,
        .source = "DEMO",
        .title = "Test Song",
        .artist = "Test Artist",
        .position_ms = 60000,
        .duration_ms = 180000,
        .volume = 72,
        .playback = PLAYBACK_PLAYING,
    };
    xSemaphoreGive(state_mutex);
}

void app_state_snapshot(media_state_t *out)
{
    assert(state_mutex != NULL && out != NULL);
    xSemaphoreTake(state_mutex, portMAX_DELAY);
    *out = state;
    xSemaphoreGive(state_mutex);
}
