#pragma once
#include "esp_err.h"
#include "app_state.h"

esp_err_t display_init(void);
esp_err_t display_render(const media_state_t *state);
