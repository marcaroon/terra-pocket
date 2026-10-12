/* Runs the shared protocol vectors (docs/protocol-test-vectors) against the C codec. */
#include "protocol/protocol.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *vector_dir;
static int failures;

#define CHECK(cond, name) do { if (!(cond)) { printf("FAIL %s: %s\n", name, #cond); ++failures; return; } } while (0)

static size_t read_file(const char *name, uint8_t *out, size_t capacity)
{
    char path[512];
    snprintf(path, sizeof(path), "%s/%s.bin", vector_dir, name);
    FILE *f = fopen(path, "rb");
    if (f == NULL) { printf("missing %s\n", path); exit(1); }
    const size_t n = fread(out, 1, capacity, f);
    assert(!ferror(f) && feof(f));
    fclose(f);
    return n;
}

/* Hex string ("-" means empty) into bytes; returns length. */
static size_t unhex(const char *hex, uint8_t *out, size_t capacity)
{
    if (strcmp(hex, "-") == 0) return 0;
    const size_t n = strlen(hex) / 2;
    assert(strlen(hex) % 2 == 0 && n <= capacity);
    for (size_t i = 0; i < n; ++i) {
        unsigned value;
        sscanf(hex + 2 * i, "%2x", &value);
        out[i] = (uint8_t)value;
    }
    return n;
}

static long long num(const char *text) { return strtoll(text, NULL, 0); }

/* Portable replacement for strtok_r: splits *cursor at delim in place. */
static char *next_token(char **cursor, char delim)
{
    char *start = *cursor;
    if (start == NULL) return NULL;
    char *end = strchr(start, delim);
    if (end != NULL) { *end = '\0'; *cursor = end + 1; } else { *cursor = NULL; }
    return start;
}

static void test_payload(const char *name, const char *kind, bool ok, char **f)
{
    uint8_t data[1024], again[1024];
    const size_t n = read_file(name, data, sizeof(data));
    size_t m = 0;
    if (strcmp(kind, "media_state") == 0) {
        protocol_media_state_t s;
        const bool decoded = protocol_decode_media_state(data, n, &s);
        CHECK(decoded == ok, name);
        if (!ok) return;
        uint8_t text[128];
        CHECK(s.playback == num(f[0]) && s.position_ms == (uint32_t)num(f[1])
              && s.duration_ms == (uint32_t)num(f[2]), name);
        const char *fields[4] = {s.source, s.title, s.artist, s.album};
        for (int i = 0; i < 4; ++i) {
            const size_t len = unhex(f[3 + i], text, sizeof(text));
            CHECK(strlen(fields[i]) == len && memcmp(fields[i], text, len) == 0, name);
        }
        m = protocol_encode_media_state(&s, again, sizeof(again));
    } else if (strcmp(kind, "volume_state") == 0) {
        protocol_volume_t v;
        CHECK(protocol_decode_volume(data, n, &v) == ok, name);
        if (!ok) return;
        CHECK(v.volume == num(f[0]) && v.muted == (num(f[1]) == 1), name);
        m = protocol_encode_volume(&v, again, sizeof(again));
    } else if (strcmp(kind, "media_command") == 0) {
        protocol_command_t c;
        CHECK(protocol_decode_command(data, n, &c) == ok, name);
        if (!ok) return;
        CHECK(c.command == num(f[0]) && c.value == num(f[1]), name);
        m = protocol_encode_command(&c, again, sizeof(again));
    } else if (strcmp(kind, "hello") == 0) {
        protocol_hello_t h;
        CHECK(protocol_decode_hello(data, n, &h) == ok, name);
        if (!ok) return;
        CHECK(h.version == num(f[0]) && h.major == num(f[1]) && h.minor == num(f[2])
              && h.patch == num(f[3]) && h.capabilities == num(f[4]), name);
        m = protocol_encode_hello(&h, again, sizeof(again));
    } else if (strcmp(kind, "hello_ack") == 0) {
        protocol_hello_ack_t a;
        CHECK(protocol_decode_hello_ack(data, n, &a) == ok, name);
        if (!ok) return;
        CHECK(a.version == num(f[0]) && a.major == num(f[1]) && a.minor == num(f[2])
              && a.patch == num(f[3]) && a.status == num(f[4]), name);
        m = protocol_encode_hello_ack(&a, again, sizeof(again));
    } else if (strcmp(kind, "device_status") == 0) {
        protocol_status_t s;
        CHECK(protocol_decode_status(data, n, &s) == ok, name);
        if (!ok) return;
        CHECK(s.version == num(f[0]) && s.major == num(f[1]) && s.minor == num(f[2])
              && s.patch == num(f[3]) && s.link == num(f[4]) && s.flags == num(f[5])
              && s.rx_errors == num(f[6]), name);
        m = protocol_encode_status(&s, again, sizeof(again));
    } else if (strcmp(kind, "error") == 0) {
        protocol_error_t e;
        CHECK(protocol_decode_error(data, n, &e) == ok, name);
        if (!ok) return;
        CHECK(e.code == num(f[0]) && e.related_type == num(f[1]) && e.related_seq == num(f[2]), name);
        m = protocol_encode_error(&e, again, sizeof(again));
    } else if (strcmp(kind, "artwork_begin") == 0) {
        protocol_artwork_begin_t b;
        CHECK(protocol_decode_artwork_begin(data, n, &b) == ok, name);
        if (!ok) return;
        CHECK(b.id == (uint32_t)num(f[0]) && b.format == num(f[1]) && b.width == num(f[2])
              && b.height == num(f[3]) && b.size == (uint32_t)num(f[4]), name);
        m = protocol_encode_artwork_begin(&b, again, sizeof(again));
    } else if (strcmp(kind, "artwork_chunk") == 0) {
        protocol_artwork_chunk_t c;
        CHECK(protocol_decode_artwork_chunk(data, n, &c) == ok, name);
        if (!ok) return;
        uint8_t expected[PROTOCOL_MAX_MESSAGE];
        const size_t len = unhex(f[2], expected, sizeof(expected));
        CHECK(c.id == (uint32_t)num(f[0]) && c.offset == (uint32_t)num(f[1]) && c.length == len
              && memcmp(c.data, expected, len) == 0, name);
        m = protocol_encode_artwork_chunk(&c, again, sizeof(again));
    } else if (strcmp(kind, "artwork_end") == 0) {
        protocol_artwork_end_t e;
        CHECK(protocol_decode_artwork_end(data, n, &e) == ok, name);
        if (!ok) return;
        CHECK(e.id == (uint32_t)strtoul(f[0], NULL, 0) && e.crc32 == (uint32_t)strtoul(f[1], NULL, 0), name);
        m = protocol_encode_artwork_end(&e, again, sizeof(again));
    } else if (strcmp(kind, "crc32") == 0) {
        CHECK(protocol_crc32(0, data, n) == (uint32_t)strtoul(f[0], NULL, 0), name);
        /* Continuing over two halves gives the same result. */
        CHECK(protocol_crc32(protocol_crc32(0, data, n / 2), data + n / 2, n - n / 2)
              == (uint32_t)strtoul(f[0], NULL, 0), name);
        return;
    } else {
        CHECK(false && "unknown kind", name);
    }
    CHECK(m == n && memcmp(again, data, n) == 0, name); /* encode reproduces the vector */
}

static void test_fragment(const char *name, char **f)
{
    uint8_t blob[2048], payload[PROTOCOL_MAX_MESSAGE], packet[256];
    const size_t n = read_file(name, blob, sizeof(blob));
    const size_t length = unhex(f[3], payload, sizeof(payload));
    const uint8_t type = (uint8_t)num(f[0]), seq = (uint8_t)num(f[1]);
    const uint16_t mtu = (uint16_t)num(f[2]);
    const size_t count = protocol_fragment_count(length, mtu);
    size_t offset = 0;
    for (size_t i = 0; i < count; ++i) {
        const size_t len = protocol_fragment_build(type, seq, payload, length, mtu, i, packet, sizeof(packet));
        CHECK(len > 0 && offset + 2 + len <= n, name);
        CHECK((size_t)(blob[offset] | (blob[offset + 1] << 8)) == len, name);
        CHECK(memcmp(blob + offset + 2, packet, len) == 0, name);
        offset += 2 + len;
    }
    CHECK(count > 0 && offset == n, name);
}

static uint8_t drop_bits(const char *text)
{
    static const char *names[] = {"BAD_PACKET", "BAD_VERSION", "INCOMPLETE", "NO_START",
                                  "MISMATCH", "TOO_LARGE", "TIMEOUT"};
    uint8_t bits = 0;
    if (strcmp(text, "-") == 0) return 0;
    char copy[96];
    snprintf(copy, sizeof(copy), "%s", text);
    char *cursor = copy;
    for (char *part = next_token(&cursor, '+'); part != NULL; part = next_token(&cursor, '+')) {
        for (int i = 0; i < 7; ++i) {
            if (strcmp(part, names[i]) == 0) bits |= (uint8_t)(1u << i);
        }
    }
    return bits;
}

static void test_stream(const char *name, char *expected)
{
    uint8_t blob[4096], payload[PROTOCOL_MAX_MESSAGE];
    const size_t n = read_file(name, blob, sizeof(blob));
    static protocol_reassembler_t r;
    protocol_reassembler_reset(&r);
    size_t offset = 0;
    char *cursor = expected;
    for (char *step = next_token(&cursor, ';'); step != NULL; step = next_token(&cursor, ';')) {
        CHECK(offset + 6 <= n, name);
        const size_t len = blob[offset] | (blob[offset + 1] << 8);
        const uint32_t time = blob[offset + 2] | (blob[offset + 3] << 8) | ((uint32_t)blob[offset + 4] << 16)
                              | ((uint32_t)blob[offset + 5] << 24);
        const protocol_rx_result_t res = protocol_reassembler_push(&r, blob + offset + 6, len, time);
        offset += 6 + len;
        char *slash = strchr(step, '/');
        CHECK(slash != NULL, name);
        *slash = '\0';
        CHECK(res.drops == drop_bits(step), name);
        const char *status = slash + 1;
        if (strcmp(status, "-") == 0) {
            CHECK(!res.complete, name);
        } else {
            unsigned type, seq;
            CHECK(sscanf(status, "%2x:%2x:", &type, &seq) == 2, name);
            const char *hex = status + 6;
            const size_t plen = *hex ? unhex(hex, payload, sizeof(payload)) : 0;
            CHECK(res.complete && res.type == type && res.seq == seq && res.length == plen, name);
            CHECK(plen == 0 || memcmp(res.payload, payload, plen) == 0, name);
        }
    }
    CHECK(offset == n, name);
}

int main(int argc, char **argv)
{
    vector_dir = argc > 1 ? argv[1] : "docs/protocol-test-vectors";
    char path[512];
    snprintf(path, sizeof(path), "%s/manifest.tsv", vector_dir);
    FILE *manifest = fopen(path, "r");
    if (manifest == NULL) { printf("missing %s\n", path); return 1; }
    static char line[8192];
    int count = 0;
    while (fgets(line, sizeof(line), manifest)) {
        line[strcspn(line, "\r\n")] = '\0';
        if (line[0] == '#' || line[0] == '\0') continue;
        char *fields[16] = {0};
        int nf = 0;
        char *cursor = line;
        for (char *tok = next_token(&cursor, '\t'); tok != NULL && nf < 16; tok = next_token(&cursor, '\t')) {
            fields[nf++] = tok;
        }
        assert(nf >= 3);
        const bool ok = strcmp(fields[2], "ok") == 0;
        if (strcmp(fields[1], "fragment") == 0) test_fragment(fields[0], fields + 3);
        else if (strcmp(fields[1], "stream") == 0) test_stream(fields[0], fields[3]);
        else test_payload(fields[0], fields[1], ok, fields + 3);
        ++count;
    }
    fclose(manifest);

    /* Edge cases outside the vector format. */
    uint8_t out[8];
    protocol_reassembler_t r;
    protocol_reassembler_reset(&r);
    assert(protocol_fragment_count(10, 7) == 0);                     /* MTU leaves no payload room */
    assert(protocol_fragment_count(PROTOCOL_MAX_MESSAGE + 1, 247) == 0);
    assert(protocol_fragment_build(0x10, 0, NULL, 0, 23, 1, out, sizeof(out)) == 0); /* index range */
    const uint8_t late[] = {1, 0x11, PROTOCOL_FLAG_START, 7, 5};
    assert(!protocol_reassembler_push(&r, late, sizeof(late), UINT32_MAX - 10).complete);
    const uint8_t end[] = {1, 0x11, PROTOCOL_FLAG_END, 7, 0};
    const protocol_rx_result_t wrap = protocol_reassembler_push(&r, end, sizeof(end), 20); /* timer wrap */
    assert(wrap.complete && wrap.drops == 0 && wrap.length == 2);
    const uint8_t foreign[] = {2, 0x10, PROTOCOL_FLAG_START, 9};
    const protocol_rx_result_t res = protocol_reassembler_push(&r, foreign, sizeof(foreign), 30);
    assert(res.drops == PROTOCOL_DROP_BAD_VERSION && res.rejected_start && res.rejected_type == 0x10
           && res.rejected_seq == 9);

    if (failures) { printf("%d protocol vector failures\n", failures); return 1; }
    printf("PASS protocol: %d shared vectors, timer wrap, foreign version\n", count);
    return 0;
}
