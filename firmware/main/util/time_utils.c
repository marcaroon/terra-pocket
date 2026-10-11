#include "time_utils.h"
#include <inttypes.h>
#include <stdio.h>

void time_format_ms(uint32_t milliseconds, char *out, size_t capacity)
{
    if (out == NULL || capacity == 0) {
        return;
    }
    uint32_t seconds = milliseconds / 1000;
    snprintf(out, capacity, "%02" PRIu32 ":%02" PRIu32,
             seconds / 60, seconds % 60);
}

uint32_t timeline_position_ms(uint32_t base_ms, uint32_t duration_ms,
                              uint32_t synced_at_ms, uint32_t now_ms, int playing)
{
    uint64_t position = base_ms;
    if (playing) {
        position += (uint32_t)(now_ms - synced_at_ms);
    }
    if (duration_ms > 0 && position > duration_ms) {
        position = duration_ms;
    }
    return position > UINT32_MAX ? UINT32_MAX : (uint32_t)position;
}
