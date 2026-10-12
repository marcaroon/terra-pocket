#pragma once

/* Receives one album artwork JPEG from ARTWORK_BEGIN/CHUNK/END (docs/ble-protocol.md
 * §4.8) into a caller-owned buffer. Pure C, host-tested; never trusts the sender. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "protocol/protocol.h"

typedef enum {
    ARTWORK_RX_CONTINUE,  /* accepted; more data expected */
    ARTWORK_RX_CLEARED,   /* the current track has no artwork */
    ARTWORK_RX_COMPLETE,  /* buffer holds `size` bytes whose CRC matched */
    ARTWORK_RX_MALFORMED, /* out of order, unknown id, size or CRC mismatch; transfer dropped */
    ARTWORK_RX_TOO_LARGE, /* announced size exceeds the buffer; transfer dropped */
} artwork_rx_status_t;

typedef struct {
    uint8_t *buffer;
    size_t capacity;
    bool active;
    uint32_t id;
    uint32_t size;
    uint32_t received;
    uint32_t crc;
} artwork_rx_t;

void artwork_rx_init(artwork_rx_t *rx, uint8_t *buffer, size_t capacity);
/* Drops a partial transfer (disconnect, track change, new handshake). */
void artwork_rx_reset(artwork_rx_t *rx);
artwork_rx_status_t artwork_rx_begin(artwork_rx_t *rx, const protocol_artwork_begin_t *begin);
artwork_rx_status_t artwork_rx_chunk(artwork_rx_t *rx, const protocol_artwork_chunk_t *chunk);
/* On ARTWORK_RX_COMPLETE the image is rx->buffer[0..rx->size) with id rx->id. */
artwork_rx_status_t artwork_rx_end(artwork_rx_t *rx, const protocol_artwork_end_t *end);
