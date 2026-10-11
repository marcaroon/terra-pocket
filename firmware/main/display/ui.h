#pragma once
#include "lvgl.h"
#include "app_state.h"

/* Caller owns the LVGL lock. Only this module creates/modifies widgets. */
void ui_render(lv_obj_t *screen, const media_state_t *state);
