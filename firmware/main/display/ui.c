#include "ui.h"
#include "util/time_utils.h"
#include <stdio.h>

static lv_obj_t *label(lv_obj_t *parent, const char *text, int x, int y,
                       int width, int height, const lv_font_t *font, uint32_t color)
{
    lv_obj_t *obj = lv_label_create(parent);
    lv_obj_set_pos(obj, x, y);
    lv_obj_set_size(obj, width, height);
    lv_obj_set_style_text_font(obj, font, 0);
    lv_obj_set_style_text_color(obj, lv_color_hex(color), 0);
    lv_label_set_long_mode(obj, LV_LABEL_LONG_MODE_DOTS);
    lv_label_set_text(obj, text);
    return obj;
}

void ui_render(lv_obj_t *screen, const media_state_t *state)
{
    lv_obj_clean(screen);
    lv_obj_remove_flag(screen, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_set_style_bg_color(screen, lv_color_hex(0x101318), 0);
    lv_obj_set_style_bg_opa(screen, LV_OPA_COVER, 0);
    lv_obj_set_style_pad_all(screen, 0, 0);

    const uint32_t foreground = 0xF3F5F7;
    const uint32_t secondary = 0xAFB8C5;
    const uint32_t accent = 0x80CFFF;
    const int width = lv_obj_get_width(screen);
    const int height = lv_obj_get_height(screen);
    const int content_width = width - 36;
    label(screen, state->source, 18, 16, content_width - 54, 20, &lv_font_montserrat_14, secondary);
    const bool ready = state->connection == CONNECTION_READY;
    label(screen, ready ? "BLE" : "OFF", width - 54, 16, 36, 20,
          &lv_font_montserrat_14, ready ? accent : secondary);
    label(screen, state->title, 18, 48, content_width, 66, &lv_font_montserrat_28, foreground);
    label(screen, state->artist, 18, height - 116, content_width, 26, &lv_font_montserrat_18, secondary);

    lv_obj_t *bar = lv_bar_create(screen);
    lv_obj_set_pos(bar, 18, height - 74);
    lv_obj_set_size(bar, content_width, 4);
    lv_bar_set_range(bar, 0, 1000);
    uint32_t position = state->position_ms > state->duration_ms
                            ? state->duration_ms : state->position_ms;
    const int progress = state->duration_ms == 0 ? 0
        : (int)((uint64_t)position * 1000 / state->duration_ms);
    lv_bar_set_value(bar, progress, LV_ANIM_OFF);
    lv_obj_set_style_bg_color(bar, lv_color_hex(0x333C48), LV_PART_MAIN);
    lv_obj_set_style_bg_opa(bar, LV_OPA_COVER, LV_PART_MAIN);
    lv_obj_set_style_bg_color(bar, lv_color_hex(accent), LV_PART_INDICATOR);

    char elapsed[TIME_TEXT_CAPACITY];
    char duration[TIME_TEXT_CAPACITY];
    char timeline[2 * TIME_TEXT_CAPACITY + 3];
    time_format_ms(position, elapsed, sizeof(elapsed));
    time_format_ms(state->duration_ms, duration, sizeof(duration));
    snprintf(timeline, sizeof(timeline), "%s / %s", elapsed, duration);
    label(screen, timeline, 18, height - 61, content_width, 20, &lv_font_montserrat_14, secondary);

    const char *playback = state->playback == PLAYBACK_PLAYING ? "Playing"
        : state->playback == PLAYBACK_PAUSED ? "Paused" : "Stopped";
    label(screen, playback, 18, height - 32, 98, 20, &lv_font_montserrat_14, foreground);
    char volume[16];
    snprintf(volume, sizeof(volume), state->muted ? "MUTED" : "VOL %u", state->volume);
    lv_obj_t *vol = label(screen, volume, width - 118, height - 32, 100, 20,
                          &lv_font_montserrat_14, foreground);
    lv_obj_set_style_text_align(vol, LV_TEXT_ALIGN_RIGHT, 0);
}
