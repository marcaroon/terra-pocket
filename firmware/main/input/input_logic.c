#include "input_logic.h"

void button_filter_init(button_filter_t *button, bool pressed, uint32_t now_ms)
{
    *button = (button_filter_t){pressed, pressed, now_ms};
}

bool button_filter_sample(button_filter_t *button, bool pressed, uint32_t now_ms,
                          uint32_t debounce_ms)
{
    if (pressed != button->candidate) {
        button->candidate = pressed;
        button->changed_ms = now_ms;
    }
    if (pressed != button->stable && (uint32_t)(now_ms - button->changed_ms) >= debounce_ms) {
        button->stable = pressed;
        return pressed;
    }
    return false;
}

static bool at_detent(const encoder_filter_t *encoder, uint8_t ab)
{
    return ab == 3 || (encoder->half_step && ab == 0);
}

void encoder_filter_init(encoder_filter_t *encoder, uint8_t ab, bool half_step)
{
    *encoder = (encoder_filter_t){.previous = ab & 3, .half_step = half_step};
    encoder->armed = at_detent(encoder, ab & 3);
}

int encoder_filter_sample(encoder_filter_t *encoder, uint8_t ab)
{
    static const int8_t transitions[16] = {
         0, -1,  1,  0,
         1,  0,  0, -1,
        -1,  0,  0,  1,
         0,  1, -1,  0,
    };
    ab &= 3;
    const uint8_t previous = encoder->previous;
    encoder->previous = ab;
    if ((previous ^ ab) == 3) {
        /* Missed/invalid transition: resynchronize at rest, never invent motion. */
        encoder->armed = false;
        encoder->travel = 0;
    }
    if (!encoder->armed) {
        encoder->armed = at_detent(encoder, ab);
        return 0;
    }
    encoder->travel += transitions[(previous << 2) | ab];
    if (!at_detent(encoder, ab)) {
        return 0;
    }
    const int8_t click = encoder->half_step ? 2 : 4;
    const int step = encoder->travel == click ? 1 : encoder->travel == -click ? -1 : 0;
    encoder->travel = 0;
    return step;
}
