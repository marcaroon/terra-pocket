#pragma once
#include <stdint.h>
#include "lvgl.h"
#include "app_state.h"

/* Caller owns the LVGL lock. Only this module creates/modifies widgets.
 * ui_create builds the widgets once; ui_update only changes what differs, so
 * it is cheap to call periodically for the local timeline (spec section 21). */
/* `artwork_buffer` holds PROTOCOL_ARTWORK_SIDE squared RGB565 pixels that the
 * caller refreshes (inside the LVGL task) whenever media_state_t.artwork_version
 * changes; NULL disables the artwork layout. */
void ui_create(lv_obj_t *screen, uint16_t *artwork_buffer);
void ui_update(const media_state_t *state, uint32_t now_ms);
