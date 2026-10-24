#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "protocol/protocol.h"

#define ARTWORK_PIXELS (PROTOCOL_ARTWORK_SIDE * PROTOCOL_ARTWORK_SIDE)

/* Decodes a baseline JPEG of exactly PROTOCOL_ARTWORK_SIDE squared pixels into RGB565 in native byte
 * order (LVGL's RGB565 layout) using the ESP32-S3 ROM TJpgDec. Returns false for
 * other sizes, progressive or damaged data; `pixels` may then be partly written.
 * Not reentrant: call from one task. */
bool artwork_decode_jpeg(const uint8_t *jpeg, size_t length, uint16_t pixels[ARTWORK_PIXELS]);
