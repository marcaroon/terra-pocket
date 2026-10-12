/* Host tests for the artwork receiver state machine. */
#include "artwork/artwork_rx.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static uint8_t buffer[1000];
static uint8_t image[600];

static protocol_artwork_chunk_t chunk(uint32_t id, uint32_t offset, size_t length)
{
    return (protocol_artwork_chunk_t){id, offset, (uint16_t)length, image + offset};
}

static artwork_rx_status_t send_all(artwork_rx_t *rx, uint32_t id, size_t size, size_t piece)
{
    const protocol_artwork_begin_t begin = {id, PROTOCOL_ARTWORK_JPEG, 160, 160, (uint32_t)size};
    artwork_rx_status_t status = artwork_rx_begin(rx, &begin);
    for (size_t offset = 0; status == ARTWORK_RX_CONTINUE && offset < size; offset += piece) {
        const size_t n = size - offset < piece ? size - offset : piece;
        const protocol_artwork_chunk_t c = chunk(id, (uint32_t)offset, n);
        status = artwork_rx_chunk(rx, &c);
    }
    if (status != ARTWORK_RX_CONTINUE) return status;
    const protocol_artwork_end_t end = {id, protocol_crc32(0, image, size)};
    return artwork_rx_end(rx, &end);
}

int main(void)
{
    for (size_t i = 0; i < sizeof(image); ++i) image[i] = (uint8_t)(i * 7 + 3);
    artwork_rx_t rx;
    artwork_rx_init(&rx, buffer, sizeof(buffer));

    /* Whole image in 232-byte chunks, as the companion sends it. */
    assert(send_all(&rx, 42, sizeof(image), 232) == ARTWORK_RX_COMPLETE);
    assert(rx.id == 42 && rx.size == sizeof(image) && memcmp(buffer, image, sizeof(image)) == 0);
    /* One-byte chunks and a single chunk work too. */
    assert(send_all(&rx, 43, 17, 1) == ARTWORK_RX_COMPLETE);
    assert(send_all(&rx, 44, 300, 300) == ARTWORK_RX_COMPLETE);

    /* Clear and capacity. */
    assert(artwork_rx_begin(&rx, &(protocol_artwork_begin_t){0}) == ARTWORK_RX_CLEARED && !rx.active);
    const protocol_artwork_begin_t huge = {7, PROTOCOL_ARTWORK_JPEG, 160, 160, sizeof(buffer) + 1};
    assert(artwork_rx_begin(&rx, &huge) == ARTWORK_RX_TOO_LARGE && !rx.active);

    /* Chunks must continue the current transfer exactly. */
    const protocol_artwork_begin_t begin = {9, PROTOCOL_ARTWORK_JPEG, 160, 160, 100};
    protocol_artwork_chunk_t c = chunk(9, 0, 50);
    assert(artwork_rx_begin(&rx, &begin) == ARTWORK_RX_CONTINUE && artwork_rx_chunk(&rx, &c) == ARTWORK_RX_CONTINUE);
    c = chunk(9, 60, 10); /* gap */
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_MALFORMED && !rx.active);
    assert(artwork_rx_begin(&rx, &begin) == ARTWORK_RX_CONTINUE);
    c = chunk(8, 0, 10); /* foreign id */
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_MALFORMED);
    assert(artwork_rx_begin(&rx, &begin) == ARTWORK_RX_CONTINUE);
    c = chunk(9, 0, 101); /* past the announced size */
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_MALFORMED);
    c = chunk(9, 0, 10); /* no transfer active */
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_MALFORMED);

    /* END needs the right id, every byte and a matching CRC. */
    protocol_artwork_end_t end = {9, protocol_crc32(0, image, 100)};
    assert(artwork_rx_begin(&rx, &begin) == ARTWORK_RX_CONTINUE);
    c = chunk(9, 0, 99);
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_CONTINUE);
    assert(artwork_rx_end(&rx, &end) == ARTWORK_RX_MALFORMED); /* one byte missing */
    assert(artwork_rx_begin(&rx, &begin) == ARTWORK_RX_CONTINUE);
    c = chunk(9, 0, 100);
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_CONTINUE);
    end.crc32 ^= 1;
    assert(artwork_rx_end(&rx, &end) == ARTWORK_RX_MALFORMED); /* corrupted */
    end.crc32 ^= 1;
    assert(artwork_rx_end(&rx, &end) == ARTWORK_RX_MALFORMED); /* transfer already dropped */

    /* A new BEGIN replaces an unfinished transfer; reset drops it. */
    assert(artwork_rx_begin(&rx, &begin) == ARTWORK_RX_CONTINUE);
    c = chunk(9, 0, 40);
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_CONTINUE);
    assert(send_all(&rx, 10, 200, 64) == ARTWORK_RX_COMPLETE);
    assert(artwork_rx_begin(&rx, &begin) == ARTWORK_RX_CONTINUE);
    artwork_rx_reset(&rx);
    c = chunk(9, 0, 10);
    assert(artwork_rx_chunk(&rx, &c) == ARTWORK_RX_MALFORMED);

    /* No buffer: every image is too large. */
    artwork_rx_t none;
    artwork_rx_init(&none, NULL, 1234);
    assert(artwork_rx_begin(&none, &begin) == ARTWORK_RX_TOO_LARGE);

    puts("PASS artwork rx: chunks, clear, capacity, gaps, foreign id, overflow, CRC, restart");
    return 0;
}
