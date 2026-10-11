/* Headless validation of the actual UI renderer; no hardware is simulated. */
#include "display/ui.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { WIDTH = PREVIEW_WIDTH, HEIGHT = PREVIEW_HEIGHT };
static uint16_t draw_buffer[WIDTH * HEIGHT];
static uint8_t pixels[HEIGHT][WIDTH][3];

static void flush(lv_display_t *display, const lv_area_t *area, uint8_t *map)
{
    const uint16_t *source = (const uint16_t *)map;
    for (int y = area->y1; y <= area->y2; ++y) {
        for (int x = area->x1; x <= area->x2; ++x) {
            uint16_t value = *source++;
            if (x < 0 || x >= WIDTH || y < 0 || y >= HEIGHT) {
                abort();
            }
            pixels[y][x][0] = ((value >> 11) & 31) * 255 / 31;
            pixels[y][x][1] = ((value >> 5) & 63) * 255 / 63;
            pixels[y][x][2] = (value & 31) * 255 / 31;
        }
    }
    lv_display_flush_ready(display);
}

int main(int argc, char **argv)
{
    if (argc != 2 && argc != 3) {
        fputs("Usage: ui-preview output.ppm [long-text|zero-duration|waiting|no-media|mismatch]\n", stderr);
        return 2;
    }
    lv_init();
    lv_display_t *display = lv_display_create(WIDTH, HEIGHT);
    lv_display_set_color_format(display, LV_COLOR_FORMAT_RGB565);
    lv_display_set_buffers(display, draw_buffer, NULL, sizeof(draw_buffer),
                           LV_DISPLAY_RENDER_MODE_FULL);
    lv_display_set_flush_cb(display, flush);
    media_state_t fixture = {
        .link = LINK_READY, .playback = PLAYBACK_PLAYING,
        .source = "Spotify", .title = "Test Song", .artist = "Test Artist",
        .position_ms = 60000, .duration_ms = 180000, .last_sync_ms = 0,
        .volume_known = true, .volume = 72,
    };
    if (argc == 3) {
        if (strcmp(argv[2], "long-text") == 0) {
            memset(fixture.title, 'W', sizeof(fixture.title) - 1);
            memset(fixture.artist, 'W', sizeof(fixture.artist) - 1);
            fixture.muted = true;
            fixture.playback = PLAYBACK_PAUSED;
        } else if (strcmp(argv[2], "zero-duration") == 0) {
            fixture.duration_ms = 0;
        } else if (strcmp(argv[2], "waiting") == 0) {
            fixture.link = LINK_ADVERTISING;
        } else if (strcmp(argv[2], "no-media") == 0) {
            fixture.playback = PLAYBACK_NO_MEDIA;
        } else if (strcmp(argv[2], "mismatch") == 0) {
            fixture.link = LINK_PROTOCOL_MISMATCH;
        } else {
            return 2;
        }
    }
    ui_create(lv_display_get_screen_active(display));
    ui_update(&fixture, 0);
    lv_refr_now(display);
    lv_obj_t *screen = lv_display_get_screen_active(display);
    for (uint32_t i = 0; i < lv_obj_get_child_count(screen); ++i) {
        lv_area_t bounds;
        lv_obj_get_coords(lv_obj_get_child(screen, i), &bounds);
        if (bounds.x1 < 0 || bounds.y1 < 0 || bounds.x2 >= WIDTH || bounds.y2 >= HEIGHT) {
            fputs("Widget exceeds panel bounds\n", stderr);
            return 1;
        }
    }
    FILE *out = fopen(argv[1], "wb");
    if (out == NULL) {
        return 1;
    }
    fprintf(out, "P6\n%d %d\n255\n", WIDTH, HEIGHT);
    const size_t written = fwrite(pixels, 1, sizeof(pixels), out);
    const int closed = fclose(out);
    lv_deinit();
    return written == sizeof(pixels) && closed == 0 ? 0 : 1;
}
