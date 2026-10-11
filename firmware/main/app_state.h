#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

typedef enum {
    CONNECTION_BOOT,
    CONNECTION_DISCONNECTED,
    CONNECTION_CONNECTED,
    CONNECTION_READY,
} connection_state_t;

typedef enum {
    PLAYBACK_STOPPED,
    PLAYBACK_PLAYING,
    PLAYBACK_PAUSED,
} playback_state_t;

typedef struct {
    connection_state_t connection;
    bool demo;
    char source[32];
    char title[128];
    char artist[96];
    char album[96];
    uint32_t position_ms;
    uint32_t duration_ms;
    uint8_t volume;
    playback_state_t playback;
    bool muted;
    uint64_t last_sync_ms;
} media_state_t;

esp_err_t app_state_init(void);
/* Deliberate M1 fixture; no live media/connection is implied. */
void app_state_load_mock(void);
void app_state_snapshot(media_state_t *out);
