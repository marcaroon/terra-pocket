#include "artwork/artwork_rx.h"
#include <string.h>

void artwork_rx_init(artwork_rx_t *rx, uint8_t *buffer, size_t capacity)
{
    *rx = (artwork_rx_t){.buffer = buffer, .capacity = buffer != NULL ? capacity : 0};
}

void artwork_rx_reset(artwork_rx_t *rx)
{
    rx->active = false;
    rx->id = rx->size = rx->received = 0;
    rx->crc = 0;
}

artwork_rx_status_t artwork_rx_begin(artwork_rx_t *rx, const protocol_artwork_begin_t *begin)
{
    /* A new BEGIN always replaces an unfinished transfer. */
    artwork_rx_reset(rx);
    if (begin->size == 0) {
        return ARTWORK_RX_CLEARED;
    }
    if (begin->size > rx->capacity) {
        return ARTWORK_RX_TOO_LARGE;
    }
    rx->active = true;
    rx->id = begin->id;
    rx->size = begin->size;
    return ARTWORK_RX_CONTINUE;
}

artwork_rx_status_t artwork_rx_chunk(artwork_rx_t *rx, const protocol_artwork_chunk_t *chunk)
{
    /* Writes are acknowledged in order, so a gap or overlap means a lost or foreign chunk. */
    if (!rx->active || chunk->id != rx->id || chunk->offset != rx->received
        || chunk->length > rx->size - rx->received) {
        artwork_rx_reset(rx);
        return ARTWORK_RX_MALFORMED;
    }
    memcpy(rx->buffer + rx->received, chunk->data, chunk->length);
    rx->crc = protocol_crc32(rx->crc, chunk->data, chunk->length);
    rx->received += chunk->length;
    return ARTWORK_RX_CONTINUE;
}

artwork_rx_status_t artwork_rx_end(artwork_rx_t *rx, const protocol_artwork_end_t *end)
{
    const bool ok = rx->active && end->id == rx->id && rx->received == rx->size && end->crc32 == rx->crc;
    if (!ok) {
        artwork_rx_reset(rx);
        return ARTWORK_RX_MALFORMED;
    }
    rx->active = false; /* keep id/size for the caller */
    return ARTWORK_RX_COMPLETE;
}
