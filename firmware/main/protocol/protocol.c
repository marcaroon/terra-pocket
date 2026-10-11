#include "protocol.h"
#include <string.h>

static uint16_t get_u16(const uint8_t *p) { return (uint16_t)(p[0] | (p[1] << 8)); }
static uint32_t get_u32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}
static void put_u16(uint8_t *p, uint16_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }
static void put_u32(uint8_t *p, uint32_t v)
{
    for (int i = 0; i < 4; ++i) p[i] = (uint8_t)(v >> (8 * i));
}

static bool continuation(uint8_t byte) { return (byte & 0xC0) == 0x80; }

static size_t bounded_length(const char *text, size_t limit)
{
    size_t n = 0;
    while (n < limit && text[n] != '\0') ++n;
    return n;
}

bool protocol_utf8_valid(const uint8_t *data, size_t length)
{
    size_t i = 0;
    while (i < length) {
        const uint8_t b = data[i];
        size_t extra;
        uint8_t low = 0x80, high = 0xBF; /* allowed range of the second byte */
        if (b == 0x00) return false;
        if (b < 0x80) { ++i; continue; }
        if (b >= 0xC2 && b <= 0xDF) extra = 1;
        else if (b == 0xE0) { extra = 2; low = 0xA0; }
        else if (b >= 0xE1 && b <= 0xEC) extra = 2;
        else if (b == 0xED) { extra = 2; high = 0x9F; }  /* no surrogates */
        else if (b >= 0xEE && b <= 0xEF) extra = 2;
        else if (b == 0xF0) { extra = 3; low = 0x90; }
        else if (b >= 0xF1 && b <= 0xF3) extra = 3;
        else if (b == 0xF4) { extra = 3; high = 0x8F; }  /* <= U+10FFFF */
        else return false;
        if (length - i <= extra) return false;
        if (data[i + 1] < low || data[i + 1] > high) return false;
        for (size_t k = 2; k <= extra; ++k) {
            if (!continuation(data[i + k])) return false;
        }
        i += extra + 1;
    }
    return true;
}

static const size_t text_limits[4] = {
    PROTOCOL_SOURCE_MAX, PROTOCOL_TITLE_MAX, PROTOCOL_ARTIST_MAX, PROTOCOL_ALBUM_MAX,
};

bool protocol_decode_media_state(const uint8_t *data, size_t length, protocol_media_state_t *out)
{
    if (data == NULL || out == NULL || length < 13) return false;
    if (data[0] > PROTOCOL_PLAYBACK_PAUSED) return false;
    size_t total = 13;
    for (int i = 0; i < 4; ++i) {
        if (data[9 + i] > text_limits[i]) return false;
        total += data[9 + i];
    }
    if (length != total) return false;
    char *texts[4] = {out->source, out->title, out->artist, out->album};
    size_t offset = 13;
    for (int i = 0; i < 4; ++i) {
        const size_t n = data[9 + i];
        if (!protocol_utf8_valid(data + offset, n)) return false;
        offset += n;
    }
    out->playback = data[0];
    out->position_ms = get_u32(data + 1);
    out->duration_ms = get_u32(data + 5);
    offset = 13;
    for (int i = 0; i < 4; ++i) {
        const size_t n = data[9 + i];
        memcpy(texts[i], data + offset, n);
        texts[i][n] = '\0';
        offset += n;
    }
    return true;
}

size_t protocol_encode_media_state(const protocol_media_state_t *in, uint8_t *out, size_t capacity)
{
    if (in == NULL || out == NULL || in->playback > PROTOCOL_PLAYBACK_PAUSED) return 0;
    const char *texts[4] = {in->source, in->title, in->artist, in->album};
    size_t lengths[4];
    size_t total = 13;
    for (int i = 0; i < 4; ++i) {
        lengths[i] = bounded_length(texts[i], text_limits[i] + 1);
        if (lengths[i] > text_limits[i]
            || !protocol_utf8_valid((const uint8_t *)texts[i], lengths[i])) return 0;
        total += lengths[i];
    }
    if (capacity < total) return 0;
    out[0] = in->playback;
    put_u32(out + 1, in->position_ms);
    put_u32(out + 5, in->duration_ms);
    size_t offset = 13;
    for (int i = 0; i < 4; ++i) {
        out[9 + i] = (uint8_t)lengths[i];
        memcpy(out + offset, texts[i], lengths[i]);
        offset += lengths[i];
    }
    return total;
}

bool protocol_decode_volume(const uint8_t *data, size_t length, protocol_volume_t *out)
{
    if (data == NULL || out == NULL || length != 2 || data[0] > 100 || data[1] > 1) return false;
    *out = (protocol_volume_t){data[0], data[1] == 1};
    return true;
}

size_t protocol_encode_volume(const protocol_volume_t *in, uint8_t *out, size_t capacity)
{
    if (in == NULL || out == NULL || capacity < 2 || in->volume > 100) return 0;
    out[0] = in->volume;
    out[1] = in->muted ? 1 : 0;
    return 2;
}

static bool command_valid(uint8_t command, int16_t value)
{
    switch (command) {
    case PROTOCOL_CMD_PLAY_PAUSE:
    case PROTOCOL_CMD_NEXT:
    case PROTOCOL_CMD_PREVIOUS:
    case PROTOCOL_CMD_MUTE_TOGGLE:
        return value == 0;
    case PROTOCOL_CMD_VOLUME_DELTA:
        return value != 0 && value >= -100 && value <= 100;
    default:
        return false; /* SEEK_DELTA is reserved in V1 */
    }
}

bool protocol_decode_command(const uint8_t *data, size_t length, protocol_command_t *out)
{
    if (data == NULL || out == NULL || length != 3) return false;
    const int16_t value = (int16_t)get_u16(data + 1);
    if (!command_valid(data[0], value)) return false;
    *out = (protocol_command_t){data[0], value};
    return true;
}

size_t protocol_encode_command(const protocol_command_t *in, uint8_t *out, size_t capacity)
{
    if (in == NULL || out == NULL || capacity < 3 || !command_valid(in->command, in->value)) return 0;
    out[0] = in->command;
    put_u16(out + 1, (uint16_t)in->value);
    return 3;
}

#define PROTOCOL_CAP_ALL (PROTOCOL_CAP_MEDIA_STATE | PROTOCOL_CAP_VOLUME_STATE | PROTOCOL_CAP_MEDIA_COMMAND)

bool protocol_decode_hello(const uint8_t *data, size_t length, protocol_hello_t *out)
{
    if (data == NULL || out == NULL || length != 6) return false;
    const uint16_t caps = get_u16(data + 4);
    if (caps & ~PROTOCOL_CAP_ALL) return false;
    *out = (protocol_hello_t){data[0], data[1], data[2], data[3], caps};
    return true;
}

size_t protocol_encode_hello(const protocol_hello_t *in, uint8_t *out, size_t capacity)
{
    if (in == NULL || out == NULL || capacity < 6 || (in->capabilities & ~PROTOCOL_CAP_ALL)) return 0;
    out[0] = in->version;
    out[1] = in->major;
    out[2] = in->minor;
    out[3] = in->patch;
    put_u16(out + 4, in->capabilities);
    return 6;
}

bool protocol_decode_hello_ack(const uint8_t *data, size_t length, protocol_hello_ack_t *out)
{
    if (data == NULL || out == NULL || length != 5 || data[4] > 1) return false;
    *out = (protocol_hello_ack_t){data[0], data[1], data[2], data[3], data[4]};
    return true;
}

size_t protocol_encode_hello_ack(const protocol_hello_ack_t *in, uint8_t *out, size_t capacity)
{
    if (in == NULL || out == NULL || capacity < 5 || in->status > 1) return 0;
    const uint8_t bytes[5] = {in->version, in->major, in->minor, in->patch, in->status};
    memcpy(out, bytes, sizeof(bytes));
    return 5;
}

bool protocol_decode_status(const uint8_t *data, size_t length, protocol_status_t *out)
{
    if (data == NULL || out == NULL || length != 8) return false;
    if (data[4] > PROTOCOL_LINK_MISMATCH || data[5] != 0) return false;
    *out = (protocol_status_t){data[0], data[1], data[2], data[3], data[4], data[5], get_u16(data + 6)};
    return true;
}

size_t protocol_encode_status(const protocol_status_t *in, uint8_t *out, size_t capacity)
{
    if (in == NULL || out == NULL || capacity < 8 || in->link > PROTOCOL_LINK_MISMATCH || in->flags != 0) {
        return 0;
    }
    const uint8_t bytes[6] = {in->version, in->major, in->minor, in->patch, in->link, in->flags};
    memcpy(out, bytes, sizeof(bytes));
    put_u16(out + 6, in->rx_errors);
    return 8;
}

static bool error_code_valid(uint8_t code)
{
    return code >= PROTOCOL_ERR_UNSUPPORTED_VERSION && code <= PROTOCOL_ERR_NOT_READY;
}

bool protocol_decode_error(const uint8_t *data, size_t length, protocol_error_t *out)
{
    if (data == NULL || out == NULL || length != 3 || !error_code_valid(data[0])) return false;
    *out = (protocol_error_t){data[0], data[1], data[2]};
    return true;
}

size_t protocol_encode_error(const protocol_error_t *in, uint8_t *out, size_t capacity)
{
    if (in == NULL || out == NULL || capacity < 3 || !error_code_valid(in->code)) return 0;
    out[0] = in->code;
    out[1] = in->related_type;
    out[2] = in->related_seq;
    return 3;
}

static size_t chunk_size(uint16_t mtu)
{
    return mtu > 3 + PROTOCOL_HEADER_LEN ? (size_t)mtu - 3 - PROTOCOL_HEADER_LEN : 0;
}

size_t protocol_fragment_count(size_t payload_length, uint16_t mtu)
{
    const size_t chunk = chunk_size(mtu);
    if (chunk == 0 || payload_length > PROTOCOL_MAX_MESSAGE) return 0;
    return payload_length == 0 ? 1 : (payload_length + chunk - 1) / chunk;
}

size_t protocol_fragment_build(uint8_t type, uint8_t seq, const uint8_t *payload,
                               size_t payload_length, uint16_t mtu, size_t index,
                               uint8_t *out, size_t capacity)
{
    const size_t count = protocol_fragment_count(payload_length, mtu);
    if (count == 0 || index >= count || out == NULL || (payload == NULL && payload_length > 0)) return 0;
    const size_t chunk = chunk_size(mtu);
    const size_t offset = index * chunk;
    const size_t piece = payload_length - offset < chunk ? payload_length - offset : chunk;
    if (capacity < PROTOCOL_HEADER_LEN + piece) return 0;
    out[0] = PROTOCOL_VERSION;
    out[1] = type;
    out[2] = (uint8_t)((index == 0 ? PROTOCOL_FLAG_START : 0) | (index == count - 1 ? PROTOCOL_FLAG_END : 0));
    out[3] = seq;
    if (piece > 0) memcpy(out + PROTOCOL_HEADER_LEN, payload + offset, piece);
    return PROTOCOL_HEADER_LEN + piece;
}

void protocol_reassembler_reset(protocol_reassembler_t *r)
{
    r->active = false;
    r->length = 0;
}

protocol_rx_result_t protocol_reassembler_push(protocol_reassembler_t *r, const uint8_t *packet,
                                               size_t length, uint32_t now_ms)
{
    protocol_rx_result_t result = {0};
    if (r->active && (uint32_t)(now_ms - r->last_ms) > PROTOCOL_REASSEMBLY_TIMEOUT_MS) {
        result.drops |= PROTOCOL_DROP_TIMEOUT;
        protocol_reassembler_reset(r);
    }
    if (packet == NULL || length < PROTOCOL_HEADER_LEN
        || (packet[2] & ~(PROTOCOL_FLAG_START | PROTOCOL_FLAG_END | PROTOCOL_FLAG_ACK_REQUEST))) {
        result.drops |= PROTOCOL_DROP_BAD_PACKET;
        return result;
    }
    const uint8_t type = packet[1], flags = packet[2], seq = packet[3];
    if (packet[0] != PROTOCOL_VERSION) {
        result.drops |= PROTOCOL_DROP_BAD_VERSION;
        result.rejected_start = (flags & PROTOCOL_FLAG_START) != 0;
        result.rejected_type = type;
        result.rejected_seq = seq;
        return result;
    }
    const size_t body = length - PROTOCOL_HEADER_LEN;
    if (flags & PROTOCOL_FLAG_START) {
        if (r->active) result.drops |= PROTOCOL_DROP_INCOMPLETE;
        r->active = true;
        r->type = type;
        r->seq = seq;
        r->length = 0;
    } else if (!r->active) {
        result.drops |= PROTOCOL_DROP_NO_START;
        return result;
    } else if (type != r->type || seq != r->seq) {
        result.drops |= PROTOCOL_DROP_MISMATCH;
        protocol_reassembler_reset(r);
        return result;
    }
    if ((size_t)r->length + body > PROTOCOL_MAX_MESSAGE) {
        result.drops |= PROTOCOL_DROP_TOO_LARGE;
        result.rejected_type = r->type;
        result.rejected_seq = r->seq;
        protocol_reassembler_reset(r);
        return result;
    }
    memcpy(r->buffer + r->length, packet + PROTOCOL_HEADER_LEN, body);
    r->length = (uint16_t)(r->length + body);
    r->last_ms = now_ms;
    if (flags & PROTOCOL_FLAG_END) {
        result.complete = true;
        result.type = r->type;
        result.seq = r->seq;
        result.length = r->length;
        result.payload = r->buffer;
        r->active = false;
    }
    return result;
}
