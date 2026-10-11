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
