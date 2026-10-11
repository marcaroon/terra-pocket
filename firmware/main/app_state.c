#include "app_state.h"
#include <assert.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"

static SemaphoreHandle_t state_mutex;
static media_state_t state;

static void lock(void)
{
    assert(state_mutex != NULL);
    xSemaphoreTake(state_mutex, portMAX_DELAY);
}

static void unlock(void)
{
    xSemaphoreGive(state_mutex);
}

static void clear_media(void)
{
    state.playback = PLAYBACK_NO_MEDIA;
    state.source[0] = state.title[0] = state.artist[0] = state.album[0] = '\0';
    state.position_ms = state.duration_ms = state.last_sync_ms = 0;
    state.volume_known = false;
    state.volume = 0;
    state.muted = false;
}

esp_err_t app_state_init(void)
{
    if (state_mutex != NULL) {
        return ESP_ERR_INVALID_STATE;
    }
    state_mutex = xSemaphoreCreateMutex();
    if (state_mutex == NULL) {
        return ESP_ERR_NO_MEM;
    }
    state = (media_state_t){.link = LINK_BOOT, .playback = PLAYBACK_NO_MEDIA};
    return ESP_OK;
}

void app_state_snapshot(media_state_t *out)
{
    assert(out != NULL);
    lock();
    *out = state;
    unlock();
}

void app_state_set_link(link_state_t link)
{
    lock();
    if (link != LINK_READY) {
        clear_media();
    }
    state.link = link;
    unlock();
}

void app_state_apply_media(const protocol_media_state_t *media, uint32_t now_ms)
{
    assert(media != NULL);
    lock();
    state.playback = (playback_state_t)media->playback;
    /* Both buffers have identical sizes; protocol strings are NUL-terminated. */
    memcpy(state.source, media->source, sizeof(state.source));
    memcpy(state.title, media->title, sizeof(state.title));
    memcpy(state.artist, media->artist, sizeof(state.artist));
    memcpy(state.album, media->album, sizeof(state.album));
    state.position_ms = media->position_ms;
    state.duration_ms = media->duration_ms;
    state.last_sync_ms = now_ms;
    unlock();
}

void app_state_apply_volume(const protocol_volume_t *volume)
{
    assert(volume != NULL);
    lock();
    state.volume_known = true;
    state.volume = volume->volume;
    state.muted = volume->muted;
    unlock();
}
