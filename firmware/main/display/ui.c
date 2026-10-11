#include "ui.h"
#include "util/time_utils.h"
#include <ctype.h>
#include <stdio.h>
#include <string.h>

#define COLOR_FOREGROUND 0xF3F5F7
#define COLOR_SECONDARY  0xAFB8C5
#define COLOR_ACCENT     0x80CFFF
#define COLOR_WARNING    0xFFB86B

static struct {
    lv_obj_t *source, *link, *title, *artist, *bar, *timeline, *playback, *volume;
} w;

static lv_obj_t *label(lv_obj_t *parent, int x, int y, int width, int height,
                       const lv_font_t *font, uint32_t color)
{
    lv_obj_t *obj = lv_label_create(parent);
    lv_obj_set_pos(obj, x, y);
    lv_obj_set_size(obj, width, height);
    lv_obj_set_style_text_font(obj, font, 0);
    lv_obj_set_style_text_color(obj, lv_color_hex(color), 0);
    lv_label_set_long_mode(obj, LV_LABEL_LONG_MODE_DOTS);
    lv_label_set_text_static(obj, "");
    return obj;
}

void ui_create(lv_obj_t *screen)
{
    lv_obj_clean(screen);
    lv_obj_remove_flag(screen, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_set_style_bg_color(screen, lv_color_hex(0x101318), 0);
    lv_obj_set_style_bg_opa(screen, LV_OPA_COVER, 0);
    lv_obj_set_style_pad_all(screen, 0, 0);

    const int width = lv_obj_get_width(screen);
    const int height = lv_obj_get_height(screen);
    const int content = width - 36;
    w.source = label(screen, 18, 16, content - 54, 20, &lv_font_montserrat_14, COLOR_SECONDARY);
    w.link = label(screen, width - 54, 16, 36, 20, &lv_font_montserrat_14, COLOR_SECONDARY);
    lv_obj_set_style_text_align(w.link, LV_TEXT_ALIGN_RIGHT, 0);
    w.title = label(screen, 18, 48, content, 66, &lv_font_montserrat_28, COLOR_FOREGROUND);
    lv_label_set_long_mode(w.title, LV_LABEL_LONG_MODE_DOTS);
    w.artist = label(screen, 18, height - 116, content, 26, &lv_font_montserrat_18, COLOR_SECONDARY);

    w.bar = lv_bar_create(screen);
    lv_obj_set_pos(w.bar, 18, height - 74);
    lv_obj_set_size(w.bar, content, 4);
    lv_bar_set_range(w.bar, 0, 1000);
    lv_obj_set_style_bg_color(w.bar, lv_color_hex(0x333C48), LV_PART_MAIN);
    lv_obj_set_style_bg_opa(w.bar, LV_OPA_COVER, LV_PART_MAIN);
    lv_obj_set_style_bg_color(w.bar, lv_color_hex(COLOR_ACCENT), LV_PART_INDICATOR);

    w.timeline = label(screen, 18, height - 61, content, 20, &lv_font_montserrat_14, COLOR_SECONDARY);
    w.playback = label(screen, 18, height - 32, 98, 20, &lv_font_montserrat_14, COLOR_FOREGROUND);
    w.volume = label(screen, width - 118, height - 32, 100, 20, &lv_font_montserrat_14, COLOR_FOREGROUND);
    lv_obj_set_style_text_align(w.volume, LV_TEXT_ALIGN_RIGHT, 0);
}

/* Sets text only when it changed, avoiding needless redraws on periodic updates. */
static void set_text(lv_obj_t *obj, const char *text)
{
    if (strcmp(lv_label_get_text(obj), text) != 0) {
        lv_label_set_text(obj, text);
    }
}

static void set_visible(lv_obj_t *obj, bool visible)
{
    if (visible == lv_obj_has_flag(obj, LV_OBJ_FLAG_HIDDEN)) {
        if (visible) lv_obj_remove_flag(obj, LV_OBJ_FLAG_HIDDEN);
        else lv_obj_add_flag(obj, LV_OBJ_FLAG_HIDDEN);
    }
}

static void set_color(lv_obj_t *obj, uint32_t color)
{
    lv_obj_set_style_text_color(obj, lv_color_hex(color), 0);
}

void ui_update(const media_state_t *state, uint32_t now_ms)
{
    const bool ready = state->link == LINK_READY;
    const bool media = ready && state->playback != PLAYBACK_NO_MEDIA;

    const char *link_text = state->link == LINK_READY ? "BLE"
        : state->link == LINK_CONNECTED ? "..."
        : state->link == LINK_PROTOCOL_MISMATCH ? "ERR" : "OFF";
    if (strcmp(lv_label_get_text(w.link), link_text) != 0) {
        lv_label_set_text(w.link, link_text);
        set_color(w.link, state->link == LINK_READY ? COLOR_ACCENT
                  : state->link == LINK_PROTOCOL_MISMATCH ? COLOR_WARNING : COLOR_SECONDARY);
    }

    if (media) {
        char source[PROTOCOL_SOURCE_MAX + 1];
        size_t i = 0;
        for (; state->source[i] != '\0' && i < sizeof(source) - 1; ++i) {
            /* ASCII-only uppercase keeps multibyte UTF-8 sequences intact. */
            const unsigned char c = (unsigned char)state->source[i];
            source[i] = c < 0x80 ? (char)toupper(c) : (char)c;
        }
        source[i] = '\0';
        set_text(w.source, source);
        set_text(w.title, state->title[0] ? state->title : "Unknown title");
        set_text(w.artist, state->artist);
    } else if (state->link == LINK_BOOT) {
        set_text(w.source, "");
        set_text(w.title, "TERRA");
        set_text(w.artist, "Starting...");
    } else if (state->link == LINK_ADVERTISING) {
        set_text(w.source, "BLUETOOTH");
        set_text(w.title, "Waiting for computer...");
        set_text(w.artist, "Terra");
    } else if (state->link == LINK_CONNECTED) {
        set_text(w.source, "BLUETOOTH");
        set_text(w.title, "Connecting...");
        set_text(w.artist, "");
    } else if (state->link == LINK_PROTOCOL_MISMATCH) {
        set_text(w.source, "BLUETOOTH");
        set_text(w.title, "Protocol mismatch");
        set_text(w.artist, "Update companion");
    } else {
        set_text(w.source, "CONNECTED");
        set_text(w.title, "Nothing Playing");
        set_text(w.artist, "");
    }

    set_visible(w.bar, media);
    set_visible(w.timeline, media);
    set_visible(w.playback, media);
    set_visible(w.volume, ready);

    if (media) {
        const uint32_t position = timeline_position_ms(state->position_ms, state->duration_ms,
                                                       state->last_sync_ms, now_ms,
                                                       state->playback == PLAYBACK_PLAYING);
        const int progress = state->duration_ms == 0 ? 0
            : (int)((uint64_t)position * 1000 / state->duration_ms);
        lv_bar_set_value(w.bar, progress, LV_ANIM_OFF);
        char elapsed[TIME_TEXT_CAPACITY];
        char duration[TIME_TEXT_CAPACITY];
        char timeline[2 * TIME_TEXT_CAPACITY + 3];
        time_format_ms(position, elapsed, sizeof(elapsed));
        if (state->duration_ms > 0) {
            time_format_ms(state->duration_ms, duration, sizeof(duration));
        } else {
            snprintf(duration, sizeof(duration), "--:--");
        }
        snprintf(timeline, sizeof(timeline), "%s / %s", elapsed, duration);
        set_text(w.timeline, timeline);
        set_text(w.playback, state->playback == PLAYBACK_PLAYING ? "Playing" : "Paused");
    }
    if (ready) {
        char volume[16];
        if (!state->volume_known) snprintf(volume, sizeof(volume), "VOL --");
        else if (state->muted) snprintf(volume, sizeof(volume), "MUTED");
        else snprintf(volume, sizeof(volume), "VOL %u", state->volume);
        set_text(w.volume, volume);
    }
}
