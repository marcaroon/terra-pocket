#include "app_state.h"
#include <assert.h>
#include <string.h>
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"

#define ARTWORK_BYTES (PROTOCOL_ARTWORK_SIDE * PROTOCOL_ARTWORK_SIDE * sizeof(uint16_t))

static SemaphoreHandle_t state_mutex;
static media_state_t state;
static uint16_t *artwork_pixels; /* PSRAM; guarded by state_mutex */

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
    state.artwork = false;
    ++state.artwork_version;
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
    artwork_pixels = heap_caps_malloc(ARTWORK_BYTES, MALLOC_CAP_SPIRAM);
    if (artwork_pixels == NULL) {
        ESP_LOGW("STATE", "No PSRAM for artwork; album art disabled");
    }
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

bool app_state_artwork_supported(void)
{
    return artwork_pixels != NULL;
}

void app_state_set_artwork(const uint16_t *pixels)
{
    assert(pixels != NULL);
    if (artwork_pixels == NULL) {
        return;
    }
    lock();
    memcpy(artwork_pixels, pixels, ARTWORK_BYTES);
    state.artwork = true;
    ++state.artwork_version;
    unlock();
}

void app_state_clear_artwork(void)
{
    lock();
    if (state.artwork) {
        state.artwork = false;
        ++state.artwork_version;
    }
    unlock();
}

bool app_state_copy_artwork(uint16_t *out)
{
    assert(out != NULL);
    lock();
    const bool present = state.artwork;
    if (present) {
        memcpy(out, artwork_pixels, ARTWORK_BYTES);
    }
    unlock();
    return present;
}

void app_state_set_view(view_t view)
{
    lock();
    state.view = view;
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
