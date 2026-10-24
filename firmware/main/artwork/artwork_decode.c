#include "artwork/artwork_decode.h"
#include <string.h>
#include "rom/tjpgd.h"

typedef struct {
    const uint8_t *data;
    size_t length;
    size_t offset;
    uint16_t *pixels;
} source_t;

/* TJpgDec reads the stream through this callback; a NULL buffer means skip. */
static UINT input(JDEC *decoder, BYTE *buffer, UINT wanted)
{
    source_t *source = decoder->device;
    const size_t left = source->length - source->offset;
    const UINT n = wanted < left ? wanted : (UINT)left;
    if (buffer != NULL) {
        memcpy(buffer, source->data + source->offset, n);
    }
    source->offset += n;
    return n;
}

/* The ROM build outputs RGB888 (JD_FORMAT 0) one MCU rectangle at a time. */
static UINT output(JDEC *decoder, void *bitmap, JRECT *rect)
{
    source_t *source = decoder->device;
    const BYTE *rgb = bitmap;
    for (unsigned y = rect->top; y <= rect->bottom; ++y) {
        for (unsigned x = rect->left; x <= rect->right; ++x, rgb += 3) {
            if (x < PROTOCOL_ARTWORK_SIDE && y < PROTOCOL_ARTWORK_SIDE) {
                source->pixels[y * PROTOCOL_ARTWORK_SIDE + x] =
                    (uint16_t)(((rgb[0] & 0xF8) << 8) | ((rgb[1] & 0xFC) << 3) | (rgb[2] >> 3));
            }
        }
    }
    return 1;
}

bool artwork_decode_jpeg(const uint8_t *jpeg, size_t length, uint16_t pixels[ARTWORK_PIXELS])
{
    /* TJpgDec R0.01 with a 512-byte input buffer needs about 3100 bytes of pool. */
    static uint8_t pool[3200];
    if (jpeg == NULL || pixels == NULL || length == 0) {
        return false;
    }
    source_t source = {.data = jpeg, .length = length, .pixels = pixels};
    JDEC decoder;
    if (jd_prepare(&decoder, input, pool, sizeof(pool), &source) != JDR_OK) {
        return false;
    }
    if (decoder.width != PROTOCOL_ARTWORK_SIDE || decoder.height != PROTOCOL_ARTWORK_SIDE) {
        return false;
    }
    return jd_decomp(&decoder, output, 0) == JDR_OK;
}
