#pragma once

#include <stdbool.h>
#include "esp_err.h"
#include "freertos/FreeRTOS.h"

typedef enum {
    INPUT_PREVIOUS,
    INPUT_PLAY_PAUSE,
    INPUT_NEXT,
    INPUT_MUTE,
    INPUT_VOLUME_UP,
    INPUT_VOLUME_DOWN,
    INPUT_VIEW_TOGGLE,
} input_event_t;

esp_err_t input_init(void);
bool input_receive(input_event_t *event, TickType_t wait);
