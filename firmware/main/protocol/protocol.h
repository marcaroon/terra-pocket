#pragma once

/* Terra BLE protocol V1 codec, framing and reassembly (docs/ble-protocol.md),
 * including the V2 album artwork messages (§4.8).
 * Pure C: no ESP-IDF dependencies, so it is unit-tested on the host against
 * docs/protocol-test-vectors. Never trusts input lengths. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define PROTOCOL_VERSION                1
#define PROTOCOL_HEADER_LEN             4
#define PROTOCOL_MAX_MESSAGE            512
#define PROTOCOL_REASSEMBLY_TIMEOUT_MS  1000

#define PROTOCOL_FLAG_START        0x01
#define PROTOCOL_FLAG_END          0x02
#define PROTOCOL_FLAG_ACK_REQUEST  0x04

typedef enum {
    PROTOCOL_MSG_HELLO = 0x01,
    PROTOCOL_MSG_HELLO_ACK = 0x02,
    PROTOCOL_MSG_MEDIA_STATE = 0x10,
    PROTOCOL_MSG_VOLUME_STATE = 0x11,
    PROTOCOL_MSG_MEDIA_COMMAND = 0x20,
    PROTOCOL_MSG_DEVICE_STATUS = 0x30,
    PROTOCOL_MSG_ERROR = 0x40,
    PROTOCOL_MSG_ARTWORK_BEGIN = 0x50,
    PROTOCOL_MSG_ARTWORK_CHUNK = 0x51,
    PROTOCOL_MSG_ARTWORK_END = 0x52,
} protocol_msg_type_t;

/* Reassembly drop reasons; several may be reported for one packet. */
enum {
    PROTOCOL_DROP_BAD_PACKET = 1 << 0,
    PROTOCOL_DROP_BAD_VERSION = 1 << 1,
    PROTOCOL_DROP_INCOMPLETE = 1 << 2,
    PROTOCOL_DROP_NO_START = 1 << 3,
    PROTOCOL_DROP_MISMATCH = 1 << 4,
    PROTOCOL_DROP_TOO_LARGE = 1 << 5,
    PROTOCOL_DROP_TIMEOUT = 1 << 6,
};

typedef enum {
    PROTOCOL_PLAYBACK_NO_MEDIA = 0,
    PROTOCOL_PLAYBACK_PLAYING = 1,
    PROTOCOL_PLAYBACK_PAUSED = 2,
} protocol_playback_t;

typedef enum {
    PROTOCOL_CMD_PLAY_PAUSE = 0x01,
    PROTOCOL_CMD_NEXT = 0x02,
    PROTOCOL_CMD_PREVIOUS = 0x03,
    PROTOCOL_CMD_VOLUME_DELTA = 0x04,
    PROTOCOL_CMD_MUTE_TOGGLE = 0x05,
    PROTOCOL_CMD_SEEK_DELTA = 0x06, /* reserved in V1 */
} protocol_command_id_t;

typedef enum {
    PROTOCOL_LINK_ADVERTISING = 0,
    PROTOCOL_LINK_CONNECTED = 1,
    PROTOCOL_LINK_READY = 2,
    PROTOCOL_LINK_MISMATCH = 3,
} protocol_link_t;

typedef enum {
    PROTOCOL_ERR_UNSUPPORTED_VERSION = 1,
    PROTOCOL_ERR_MALFORMED = 2,
    PROTOCOL_ERR_UNSUPPORTED_TYPE = 3,
    PROTOCOL_ERR_TOO_LARGE = 4,
    PROTOCOL_ERR_NOT_READY = 5,
} protocol_error_code_t;

#define PROTOCOL_SOURCE_MAX 31
#define PROTOCOL_TITLE_MAX  127
#define PROTOCOL_ARTIST_MAX 95
#define PROTOCOL_ALBUM_MAX  95

#define PROTOCOL_CAP_MEDIA_STATE   0x0001
#define PROTOCOL_CAP_VOLUME_STATE  0x0002
#define PROTOCOL_CAP_MEDIA_COMMAND 0x0004
#define PROTOCOL_CAP_ARTWORK       0x0008

#define PROTOCOL_ARTWORK_JPEG         1
#define PROTOCOL_ARTWORK_SIDE         240 /* full screen */
#define PROTOCOL_ARTWORK_MAX_BYTES    32768
#define PROTOCOL_ARTWORK_CHUNK_HEADER 8

typedef struct {
    uint8_t playback;
    uint32_t position_ms;
    uint32_t duration_ms;
    /* NUL-terminated copies; the wire format carries no terminator. */
    char source[PROTOCOL_SOURCE_MAX + 1];
    char title[PROTOCOL_TITLE_MAX + 1];
    char artist[PROTOCOL_ARTIST_MAX + 1];
    char album[PROTOCOL_ALBUM_MAX + 1];
} protocol_media_state_t;

typedef struct { uint8_t volume; bool muted; } protocol_volume_t;
typedef struct { uint8_t command; int16_t value; } protocol_command_t;
typedef struct { uint8_t version, major, minor, patch; uint16_t capabilities; } protocol_hello_t;
typedef struct { uint8_t version, major, minor, patch, status; } protocol_hello_ack_t;
typedef struct {
    uint8_t version, major, minor, patch, link, flags;
    uint16_t rx_errors;
} protocol_status_t;
typedef struct { uint8_t code, related_type, related_seq; } protocol_error_t;
/* size 0 (with every other field 0) means the current track has no artwork. */
typedef struct {
    uint32_t id;
    uint8_t format;
    uint16_t width, height;
    uint32_t size;
} protocol_artwork_begin_t;
typedef struct {
    uint32_t id;
    uint32_t offset;
    uint16_t length;
    const uint8_t *data; /* points into the decoded payload */
} protocol_artwork_chunk_t;
typedef struct { uint32_t id, crc32; } protocol_artwork_end_t;

/* Payload codecs. Decoders return false for any malformed payload. Encoders
 * return the number of bytes written, or 0 if the value is invalid or the
 * output buffer is too small. */
bool protocol_utf8_valid(const uint8_t *data, size_t length);
bool protocol_decode_media_state(const uint8_t *data, size_t length, protocol_media_state_t *out);
size_t protocol_encode_media_state(const protocol_media_state_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_volume(const uint8_t *data, size_t length, protocol_volume_t *out);
size_t protocol_encode_volume(const protocol_volume_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_command(const uint8_t *data, size_t length, protocol_command_t *out);
size_t protocol_encode_command(const protocol_command_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_hello(const uint8_t *data, size_t length, protocol_hello_t *out);
size_t protocol_encode_hello(const protocol_hello_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_hello_ack(const uint8_t *data, size_t length, protocol_hello_ack_t *out);
size_t protocol_encode_hello_ack(const protocol_hello_ack_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_status(const uint8_t *data, size_t length, protocol_status_t *out);
size_t protocol_encode_status(const protocol_status_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_error(const uint8_t *data, size_t length, protocol_error_t *out);
size_t protocol_encode_error(const protocol_error_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_artwork_begin(const uint8_t *data, size_t length, protocol_artwork_begin_t *out);
size_t protocol_encode_artwork_begin(const protocol_artwork_begin_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_artwork_chunk(const uint8_t *data, size_t length, protocol_artwork_chunk_t *out);
size_t protocol_encode_artwork_chunk(const protocol_artwork_chunk_t *in, uint8_t *out, size_t capacity);
bool protocol_decode_artwork_end(const uint8_t *data, size_t length, protocol_artwork_end_t *out);
size_t protocol_encode_artwork_end(const protocol_artwork_end_t *in, uint8_t *out, size_t capacity);
/* CRC-32/ISO-HDLC (zlib). Pass the previous result to continue over several buffers. */
uint32_t protocol_crc32(uint32_t crc, const uint8_t *data, size_t length);

/* Fragmentation for an ATT MTU (packet length = MTU - 3). Returns the packet
 * count, or 0 if the payload is too large or the MTU too small. */
size_t protocol_fragment_count(size_t payload_length, uint16_t mtu);
/* Builds packet `index` of a message; returns its length or 0 on error. */
size_t protocol_fragment_build(uint8_t type, uint8_t seq, const uint8_t *payload,
                               size_t payload_length, uint16_t mtu, size_t index,
                               uint8_t *out, size_t capacity);

typedef struct {
    bool active;
    uint8_t type;
    uint8_t seq;
    uint16_t length;
    uint32_t last_ms;
    uint8_t buffer[PROTOCOL_MAX_MESSAGE];
} protocol_reassembler_t;

typedef struct {
    uint8_t drops;          /* PROTOCOL_DROP_* bits */
    bool complete;          /* a whole message is available */
    uint8_t type;
    uint8_t seq;
    uint16_t length;
    const uint8_t *payload; /* points into the reassembler; valid until next push */
    /* Header of a rejected packet with a foreign version (for ERROR replies). */
    bool rejected_start;
    uint8_t rejected_type;
    uint8_t rejected_seq;
} protocol_rx_result_t;

void protocol_reassembler_reset(protocol_reassembler_t *reassembler);
protocol_rx_result_t protocol_reassembler_push(protocol_reassembler_t *reassembler,
                                               const uint8_t *packet, size_t length,
                                               uint32_t now_ms);
