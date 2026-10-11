#pragma once
#include <stddef.h>
#include <stdint.h>

/* UINT32_MAX milliseconds fits in "71582:47" plus NUL. */
#define TIME_TEXT_CAPACITY 9
void time_format_ms(uint32_t milliseconds, char *out, size_t capacity);
