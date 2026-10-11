#pragma once

#include <stdbool.h>
#include <stdint.h>

typedef struct {
    bool stable;
    bool candidate;
    uint32_t changed_ms;
} button_filter_t;

void button_filter_init(button_filter_t *button, bool pressed, uint32_t now_ms);
bool button_filter_sample(button_filter_t *button, bool pressed, uint32_t now_ms,
                          uint32_t debounce_ms);

typedef struct {
    uint8_t previous;
    int8_t travel;
    bool armed;
    bool half_step;
} encoder_filter_t;

/* Full-step: both contacts open (AB=3) at each detent, four transitions per click.
 * Half-step (e.g. KY-040): detents at AB=3 and AB=0, two transitions per click. */
void encoder_filter_init(encoder_filter_t *encoder, uint8_t ab, bool half_step);
int encoder_filter_sample(encoder_filter_t *encoder, uint8_t ab);
