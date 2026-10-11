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
    puts("time_utils: 7 checks passed");
    return 0;
}
