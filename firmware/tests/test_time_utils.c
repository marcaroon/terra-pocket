#include "util/time_utils.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

int main(void)
{
    char text[TIME_TEXT_CAPACITY];
    time_format_ms(0, text, sizeof(text));
    assert(strcmp(text, "00:00") == 0);
    time_format_ms(60000, text, sizeof(text));
    assert(strcmp(text, "01:00") == 0);
    time_format_ms(180000, text, sizeof(text));
    assert(strcmp(text, "03:00") == 0);
    time_format_ms(59999, text, sizeof(text));
    assert(strcmp(text, "00:59") == 0);
    time_format_ms(UINT32_MAX, text, sizeof(text));
    assert(strcmp(text, "71582:47") == 0);
    char bounded[] = {'x', 'x', 'x', '!', '\0'};
    time_format_ms(UINT32_MAX, bounded, 3);
    assert(bounded[2] == '\0' && bounded[3] == '!');
    time_format_ms(0, NULL, 0);

    assert(timeline_position_ms(81000, 216000, 1000, 3500, 1) == 83500);   /* advances while playing */
    assert(timeline_position_ms(81000, 216000, 1000, 3500, 0) == 81000);   /* paused: frozen */
    assert(timeline_position_ms(215000, 216000, 0, 5000, 1) == 216000);    /* clamped at duration */
    assert(timeline_position_ms(5000, 3000, 0, 0, 0) == 3000);             /* position > duration */
    assert(timeline_position_ms(1000, 0, 0, 4000, 1) == 5000);             /* unknown duration */
    assert(timeline_position_ms(0, 0, UINT32_MAX - 99, 100, 1) == 200);    /* timer wrap */
    assert(timeline_position_ms(UINT32_MAX - 5, 0, 0, 100, 1) == UINT32_MAX); /* saturates */
    puts("time_utils: 14 checks passed");
    return 0;
}
