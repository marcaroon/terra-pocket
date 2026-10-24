#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "protocol/protocol.h"

/* Link state and playback are deliberately separate enums (spec section 30). */
typedef enum {
    LINK_BOOT,
    LINK_ADVERTISING,
    LINK_CONNECTED,
    LINK_READY,
    LINK_PROTOCOL_MISMATCH,
} link_state_t;

/* Which screen shows a track that has artwork; chosen on the device, kept in NVS. */
typedef enum {
    VIEW_TEXT = 0,
    VIEW_ARTWORK = 1,
} view_t;

/* Values match the MEDIA_STATE wire encoding. */
typedef enum {
    PLAYBACK_NO_MEDIA = PROTOCOL_PLAYBACK_NO_MEDIA,
    PLAYBACK_PLAYING = PROTOCOL_PLAYBACK_PLAYING,
    PLAYBACK_PAUSED = PROTOCOL_PLAYBACK_PAUSED,
} playback_state_t;

typedef struct {
    link_state_t link;
    playback_state_t playback;
    char source[PROTOCOL_SOURCE_MAX + 1];
    char title[PROTOCOL_TITLE_MAX + 1];
    char artist[PROTOCOL_ARTIST_MAX + 1];
    char album[PROTOCOL_ALBUM_MAX + 1];
    uint32_t position_ms;   /* base position at last_sync_ms */
    uint32_t duration_ms;
    uint32_t last_sync_ms;  /* monotonic ms when position_ms was received */
    bool volume_known;
    uint8_t volume;
    bool muted;
    /* V2: a decoded 160x160 artwork for the current track is available. The
     * pixels stay in app_state; the version changes whenever they do. */
    bool artwork;
    uint32_t artwork_version;
    view_t view; /* not cleared by link changes */
} media_state_t;

esp_err_t app_state_init(void);
void app_state_snapshot(media_state_t *out);

/* Changing the link clears media and volume unless the new state is READY,
 * so a reconnect never shows stale metadata. */
void app_state_set_link(link_state_t link);
void app_state_apply_media(const protocol_media_state_t *media, uint32_t now_ms);
void app_state_apply_volume(const protocol_volume_t *volume);

/* Artwork pixels are RGB565, PROTOCOL_ARTWORK_SIDE squared. Setting copies them;
 * without a PSRAM buffer (allocation failed at init) artwork is never shown. */
bool app_state_artwork_supported(void);
void app_state_set_artwork(const uint16_t *pixels);
void app_state_clear_artwork(void);
/* Copies the current artwork into `out` and returns true, or returns false
 * when there is none. */
bool app_state_copy_artwork(uint16_t *out);
void app_state_set_view(view_t view);
