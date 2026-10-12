#include "input/input_logic.h"
#include <assert.h>
#include <stdio.h>

static int sequence(encoder_filter_t *encoder, const uint8_t *states, size_t count)
{
    int result = 0;
    for (size_t i = 0; i < count; ++i) result += encoder_filter_sample(encoder, states[i]);
    return result;
}
#define RUN(...) sequence(&encoder, (const uint8_t[]){__VA_ARGS__}, sizeof((const uint8_t[]){__VA_ARGS__}))

int main(void)
{
    button_filter_t button;
    button_filter_init(&button, false, 0);
    assert(!button_filter_sample(&button, true, 10, 40));
    assert(!button_filter_sample(&button, false, 15, 40));
    assert(!button_filter_sample(&button, true, 20, 40));
    assert(!button_filter_sample(&button, true, 59, 40));
    assert(button_filter_sample(&button, true, 60, 40));
    assert(!button_filter_sample(&button, true, 10000, 40)); // no repeat while held
    assert(!button_filter_sample(&button, false, 10001, 40));
    assert(!button_filter_sample(&button, true, 10010, 40)); // release bounce
    assert(!button_filter_sample(&button, false, 10020, 40));
    assert(!button_filter_sample(&button, false, 10060, 40));
    assert(!button_filter_sample(&button, true, 10070, 40));
    assert(button_filter_sample(&button, true, 10110, 40));
    button_filter_init(&button, true, 0); // held at boot does not send a command
    assert(!button_filter_sample(&button, true, 100, 40));
    button_filter_init(&button, false, UINT32_MAX - 30);
    assert(!button_filter_sample(&button, true, UINT32_MAX - 20, 40));
    assert(!button_filter_sample(&button, true, 18, 40));
    assert(button_filter_sample(&button, true, 19, 40)); // timer rollover

    encoder_filter_t encoder;
    encoder_filter_init(&encoder, 3, false);
    assert(RUN(1, 0, 2, 3) == 1);
    assert(RUN(2, 0, 1, 3) == -1);
    assert(RUN(1, 3, 1, 0, 1, 0, 2, 0, 2, 3) == 1); // contact bounce
    assert(RUN(1, 0, 1, 3) == 0); // reverse before full detent
    assert(RUN(0, 2, 3) == 0); // simultaneous bit change, re-anchor
    assert(RUN(1, 0, 2, 3) == 1);
    encoder_filter_init(&encoder, 0, false);
    assert(RUN(2, 3) == 0); // power-on between detents
    for (int i = 0; i < 1000; ++i) assert(RUN(1, 0, 2, 3) == 1);

    /* Half-step: sequence captured from the user's encoder on 2026-10-11. */
    encoder_filter_init(&encoder, 3, true);
    assert(RUN(1, 0) == 1);            // CW click to AB=00 detent
    assert(RUN(2, 3) == 1);            // CW click back to AB=11 detent
    assert(RUN(1, 0, 2, 3, 1, 0) == 3); // five CW clicks total
    assert(RUN(1, 3, 2, 0, 1, 3, 2, 0) == -4); // CCW clicks
    assert(RUN(1, 0) == 0);            // click started then reversed: no step
    assert(RUN(2, 0, 2, 3) == 1);      // first-contact bounce, then one CW click
    assert(RUN(0) == 0);               // simultaneous bit change, re-anchor at AB=00
    assert(RUN(2, 3) == 1);
    encoder_filter_init(&encoder, 1, true);
    assert(RUN(0) == 0);               // power-on between detents arms at AB=00
    assert(RUN(2, 3) == 1);
    press_timer_t press;
    press_timer_init(&press, false);
    assert(press_timer_sample(&press, false, 0, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, true, 100, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, true, 699, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, false, 699, 600) == PRESS_SHORT); // released before 600 ms
    assert(press_timer_sample(&press, false, 800, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, true, 1000, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, true, 1600, 600) == PRESS_LONG);  // fires while held
    assert(press_timer_sample(&press, true, 5000, 600) == PRESS_NONE);  // once
    assert(press_timer_sample(&press, false, 5001, 600) == PRESS_NONE); // no mute after a long press
    press_timer_init(&press, true);                                     // held at boot
    assert(press_timer_sample(&press, true, 2000, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, false, 2001, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, true, UINT32_MAX - 100, 600) == PRESS_NONE);
    assert(press_timer_sample(&press, true, 498, 600) == PRESS_NONE);   // timer rollover: 599 ms held
    assert(press_timer_sample(&press, true, 499, 600) == PRESS_LONG);   // 600 ms
    puts("PASS input: bounce, hold/release, rollover, direction, reversal, invalid transitions, half-step, long press");
    return 0;
}
