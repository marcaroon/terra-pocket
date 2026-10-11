#pragma once
#include "esp_err.h"
#include "app_state.h"

esp_err_t display_init(void);
/* Builds the UI, renders the first frame, turns the backlight on and starts a
 * periodic refresh from app_state inside the LVGL task. */
esp_err_t display_start_ui(void);
