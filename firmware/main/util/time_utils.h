#pragma once
#include <stddef.h>
#include <stdint.h>

/* UINT32_MAX milliseconds fits in "71582:47" plus NUL. */
#define TIME_TEXT_CAPACITY 9
void time_format_ms(uint32_t milliseconds, char *out, size_t capacity);

/* Displayed playback position (spec section 21). While playing, the base position
 * advances by the monotonic time since it was received; paused/stopped it does
 * not. The result is clamped to duration when the duration is known (> 0).
 * Timer wrap-around is handled by unsigned subtraction. */
uint32_t timeline_position_ms(uint32_t base_ms, uint32_t duration_ms,
                              uint32_t synced_at_ms, uint32_t now_ms, int playing);
