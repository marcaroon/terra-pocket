#include "ble/ble_transport.h"
#include <stdio.h>
#include <stdatomic.h>
#include <string.h>
#include "app_state.h"
#include "artwork/artwork_decode.h"
#include "artwork/artwork_rx.h"
#include "ble/ble_service.h"
#include "esp_app_desc.h"
#include "esp_check.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "protocol/protocol.h"

static const char *TAG = "PROTOCOL";

#define EVENT_QUEUE_DEPTH  16
#define NOTIFY_RETRIES     5
#define CAPABILITIES       (PROTOCOL_CAP_MEDIA_STATE | PROTOCOL_CAP_VOLUME_STATE | PROTOCOL_CAP_MEDIA_COMMAND)
#define TASK_STACK         6144 /* TJpgDec runs in this task when artwork completes */
#define COMMAND_MAX_AGE_MS 500

typedef struct {
    bool local_command;
    union {
        ble_event_t ble;
        struct { uint32_t epoch, created_ms; uint8_t payload[3]; } command;
    };
} transport_event_t;

static QueueHandle_t events;
static atomic_uint sink_drops;
static atomic_uint ready_epoch;
static uint32_t epoch_counter; /* owned by transport task; zero means not ready */

/* V2 artwork, owned by the transport task. NULL buffers disable the feature. */
static artwork_rx_t artwork;
static uint16_t *artwork_pixels;

static struct {
    bool connected;
    bool subscribed;
    uint16_t conn_handle;
    uint16_t mtu;
    link_state_t link;
    uint8_t tx_seq;
    uint16_t rx_errors;
    uint8_t firmware[3];
    protocol_reassembler_t rx;
} link;

static uint32_t now_ms(void)
{
    return (uint32_t)(esp_timer_get_time() / 1000);
}

static bool sink(const ble_event_t *event, bool must_deliver)
{
    /* Invalidate local commands immediately, before lifecycle events are dequeued. */
    if (event->type == BLE_EVENT_DISCONNECTED || event->type == BLE_EVENT_TX_UNSUBSCRIBED
        || event->type == BLE_EVENT_TX_SUBSCRIBED || event->type == BLE_EVENT_CONNECTED) {
        atomic_store(&ready_epoch, 0);
    }
    const transport_event_t queued = {.ble = *event};
    if (xQueueSend(events, &queued, must_deliver ? pdMS_TO_TICKS(50) : 0) != pdTRUE) {
        atomic_fetch_add(&sink_drops, 1);
        return false;
    }
    return true;
}

static uint8_t wire_link(link_state_t state)
{
    switch (state) {
    case LINK_CONNECTED: return PROTOCOL_LINK_CONNECTED;
    case LINK_READY: return PROTOCOL_LINK_READY;
    case LINK_PROTOCOL_MISMATCH: return PROTOCOL_LINK_MISMATCH;
    default: return PROTOCOL_LINK_ADVERTISING;
    }
}

static void publish_status(bool notify)
{
    const protocol_status_t status = {
        .version = PROTOCOL_VERSION,
        .major = link.firmware[0], .minor = link.firmware[1], .patch = link.firmware[2],
        .link = wire_link(link.link),
        .rx_errors = link.rx_errors,
    };
    uint8_t value[8];
    const size_t length = protocol_encode_status(&status, value, sizeof(value));
    if (length > 0) {
        ble_service_set_status(value, length, notify);
    }
}

static void set_link(link_state_t state)
{
    if (state == link.link) {
        return;
    }
    link.link = state;
    if (state == LINK_READY) {
        if (++epoch_counter == 0) ++epoch_counter;
        atomic_store(&ready_epoch, epoch_counter);
    } else {
        atomic_store(&ready_epoch, 0);
    }
    app_state_set_link(state);
    publish_status(true);
}

static void count_error(void)
{
    if (link.rx_errors < UINT16_MAX) {
        ++link.rx_errors;
    }
    publish_status(false);
}

static void send_message(uint8_t type, const uint8_t *payload, size_t length)
{
    if (!link.connected || !link.subscribed) {
        return;
    }
    const size_t count = protocol_fragment_count(length, link.mtu);
    if (count == 0) {
        ESP_LOGE(TAG, "Cannot fragment type 0x%02x (%u bytes, MTU %u)", type, (unsigned)length, link.mtu);
        return;
    }
    const uint8_t seq = link.tx_seq++;
    for (size_t i = 0; i < count; ++i) {
        uint8_t packet[BLE_PACKET_MAX];
        const size_t n = protocol_fragment_build(type, seq, payload, length, link.mtu, i, packet, sizeof(packet));
        esp_err_t err = ESP_FAIL;
        for (int attempt = 0; n > 0 && attempt < NOTIFY_RETRIES && err != ESP_OK; ++attempt) {
            err = ble_service_notify_tx(link.conn_handle, packet, n);
            if (err != ESP_OK) {
                vTaskDelay(pdMS_TO_TICKS(10)); /* host buffers exhausted; let them drain */
            }
        }
        if (err != ESP_OK) {
            ESP_LOGW(TAG, "Notify failed; message 0x%02x seq %u abandoned", type, seq);
            return;
        }
    }
}

static void send_error(uint8_t code, uint8_t related_type, uint8_t related_seq)
{
    const protocol_error_t error = {code, related_type, related_seq};
    uint8_t payload[3];
    const size_t n = protocol_encode_error(&error, payload, sizeof(payload));
    ESP_LOGW(TAG, "Rejected type 0x%02x seq %u (error %u)", related_type, related_seq, code);
    send_message(PROTOCOL_MSG_ERROR, payload, n);
}

static void send_hello(void)
{
    const protocol_hello_t hello = {
        .version = PROTOCOL_VERSION,
        .major = link.firmware[0], .minor = link.firmware[1], .patch = link.firmware[2],
        .capabilities = CAPABILITIES | (artwork_pixels != NULL ? PROTOCOL_CAP_ARTWORK : 0),
    };
    uint8_t payload[6];
    const size_t n = protocol_encode_hello(&hello, payload, sizeof(payload));
    ESP_LOGI(TAG, "HELLO protocol=%u firmware=%u.%u.%u", PROTOCOL_VERSION,
             link.firmware[0], link.firmware[1], link.firmware[2]);
    send_message(PROTOCOL_MSG_HELLO, payload, n);
}

static void reject(uint8_t code, uint8_t type, uint8_t seq)
{
    count_error();
    send_error(code, type, seq);
}

static void handle_artwork(uint8_t type, uint8_t seq, const uint8_t *payload, size_t length)
{
    if (artwork_pixels == NULL) {
        reject(PROTOCOL_ERR_UNSUPPORTED_TYPE, type, seq); /* HELLO did not offer it */
        return;
    }
    if (link.link != LINK_READY) {
        reject(PROTOCOL_ERR_NOT_READY, type, seq);
        return;
    }
    artwork_rx_status_t status = ARTWORK_RX_MALFORMED;
    if (type == PROTOCOL_MSG_ARTWORK_BEGIN) {
        protocol_artwork_begin_t begin;
        if (protocol_decode_artwork_begin(payload, length, &begin)) status = artwork_rx_begin(&artwork, &begin);
        else artwork_rx_reset(&artwork);
    } else if (type == PROTOCOL_MSG_ARTWORK_CHUNK) {
        protocol_artwork_chunk_t chunk;
        if (protocol_decode_artwork_chunk(payload, length, &chunk)) status = artwork_rx_chunk(&artwork, &chunk);
        else artwork_rx_reset(&artwork);
    } else {
        protocol_artwork_end_t end;
        if (protocol_decode_artwork_end(payload, length, &end)) status = artwork_rx_end(&artwork, &end);
        else artwork_rx_reset(&artwork);
    }
    switch (status) {
    case ARTWORK_RX_CONTINUE:
        return;
    case ARTWORK_RX_CLEARED:
        app_state_clear_artwork();
        ESP_LOGI("STATE", "No artwork for this track");
        return;
    case ARTWORK_RX_COMPLETE:
        if (artwork_decode_jpeg(artwork.buffer, artwork.size, artwork_pixels)) {
            app_state_set_artwork(artwork_pixels);
            ESP_LOGI("STATE", "Artwork %08lx shown (%lu bytes JPEG)",
                     (unsigned long)artwork.id, (unsigned long)artwork.size);
        } else {
            ESP_LOGW(TAG, "Artwork %08lx is not a decodable 160x160 baseline JPEG",
                     (unsigned long)artwork.id);
            reject(PROTOCOL_ERR_MALFORMED, type, seq);
        }
        return;
    case ARTWORK_RX_TOO_LARGE:
        reject(PROTOCOL_ERR_TOO_LARGE, type, seq);
        return;
    case ARTWORK_RX_MALFORMED:
        reject(PROTOCOL_ERR_MALFORMED, type, seq);
        return;
    }
}

static void handle_message(uint8_t type, uint8_t seq, const uint8_t *payload, size_t length)
{
    switch (type) {
    case PROTOCOL_MSG_HELLO_ACK: {
        if (!link.connected || !link.subscribed || link.link != LINK_CONNECTED) {
            reject(PROTOCOL_ERR_NOT_READY, type, seq);
            return;
        }
        protocol_hello_ack_t ack;
        if (!protocol_decode_hello_ack(payload, length, &ack)) {
            reject(PROTOCOL_ERR_MALFORMED, type, seq);
            return;
        }
        if (ack.version == PROTOCOL_VERSION && ack.status == 0) {
            ESP_LOGI(TAG, "HELLO_ACK protocol=%u companion=%u.%u.%u",
                     ack.version, ack.major, ack.minor, ack.patch);
            set_link(LINK_READY);
        } else {
            ESP_LOGW(TAG, "Protocol mismatch: companion protocol=%u status=%u", ack.version, ack.status);
            set_link(LINK_PROTOCOL_MISMATCH);
        }
        return;
    }
    case PROTOCOL_MSG_MEDIA_STATE: {
        if (link.link != LINK_READY) {
            reject(PROTOCOL_ERR_NOT_READY, type, seq);
            return;
        }
        static protocol_media_state_t media; /* 360 bytes; only this task uses it */
        if (!protocol_decode_media_state(payload, length, &media)) {
            reject(PROTOCOL_ERR_MALFORMED, type, seq);
            return;
        }
        media_state_t current;
        app_state_snapshot(&current);
        const bool track_changed = strcmp(current.title, media.title) != 0
            || strcmp(current.artist, media.artist) != 0 || strcmp(current.source, media.source) != 0;
        app_state_apply_media(&media, now_ms());
        if (track_changed) {
            /* Artwork belongs to one track; the companion sends the new one next. */
            artwork_rx_reset(&artwork);
            app_state_clear_artwork();
            ESP_LOGI("STATE", "Track changed: %s - %s (%s)", media.title, media.artist, media.source);
        }
        ESP_LOGD("STATE", "Playback %u at %lu/%lu ms", media.playback,
                 (unsigned long)media.position_ms, (unsigned long)media.duration_ms);
        return;
    }
    case PROTOCOL_MSG_VOLUME_STATE: {
        if (link.link != LINK_READY) {
            reject(PROTOCOL_ERR_NOT_READY, type, seq);
            return;
        }
        protocol_volume_t volume;
        if (!protocol_decode_volume(payload, length, &volume)) {
            reject(PROTOCOL_ERR_MALFORMED, type, seq);
            return;
        }
        app_state_apply_volume(&volume);
        ESP_LOGI("STATE", "Volume %u%s", volume.volume, volume.muted ? " (muted)" : "");
        return;
    }
    case PROTOCOL_MSG_ARTWORK_BEGIN:
    case PROTOCOL_MSG_ARTWORK_CHUNK:
    case PROTOCOL_MSG_ARTWORK_END:
        handle_artwork(type, seq, payload, length);
        return;
    case PROTOCOL_MSG_ERROR: {
        protocol_error_t error;
        if (protocol_decode_error(payload, length, &error)) {
            ESP_LOGW(TAG, "Companion reported error %u for type 0x%02x seq %u",
                     error.code, error.related_type, error.related_seq);
        } else {
            count_error(); /* never answer an ERROR with an ERROR */
        }
        return;
    }
    default:
        reject(PROTOCOL_ERR_UNSUPPORTED_TYPE, type, seq);
        return;
    }
}

static void handle_packet(const ble_event_t *event)
{
    if (!link.connected || event->conn_handle != link.conn_handle) {
        return;
    }
    const protocol_rx_result_t result =
        protocol_reassembler_push(&link.rx, event->data, event->value, now_ms());
    if (result.drops != 0) {
        ESP_LOGW(TAG, "Invalid packet ignored (reasons 0x%02x)", result.drops);
        count_error();
        if ((result.drops & PROTOCOL_DROP_BAD_VERSION) && result.rejected_start) {
            send_error(PROTOCOL_ERR_UNSUPPORTED_VERSION, result.rejected_type, result.rejected_seq);
        } else if (result.drops & PROTOCOL_DROP_TOO_LARGE) {
            send_error(PROTOCOL_ERR_TOO_LARGE, result.rejected_type, result.rejected_seq);
        }
    }
    if (result.complete) {
        handle_message(result.type, result.seq, result.payload, result.length);
    }
}

static void handle_event(const ble_event_t *event)
{
    if (event->type != BLE_EVENT_PACKET && event->type != BLE_EVENT_MTU) {
        artwork_rx_reset(&artwork); /* a transfer never spans a lifecycle change */
    }
    switch (event->type) {
    case BLE_EVENT_CONNECTED:
        link.connected = true;
        link.subscribed = false;
        link.conn_handle = event->conn_handle;
        link.mtu = BLE_DEFAULT_MTU;
        protocol_reassembler_reset(&link.rx);
        set_link(LINK_CONNECTED);
        break;
    case BLE_EVENT_DISCONNECTED:
        link.connected = false;
        link.subscribed = false;
        protocol_reassembler_reset(&link.rx);
        set_link(LINK_ADVERTISING);
        break;
    case BLE_EVENT_MTU:
        if (link.connected && event->conn_handle == link.conn_handle) {
            link.mtu = event->value > BLE_PACKET_MAX + 3 ? BLE_PACKET_MAX + 3 : event->value;
        }
        break;
    case BLE_EVENT_TX_SUBSCRIBED:
        if (link.connected && event->conn_handle == link.conn_handle) {
            link.subscribed = true;
            protocol_reassembler_reset(&link.rx);
            if (link.link != LINK_CONNECTED) {
                set_link(LINK_CONNECTED); /* a re-subscribe restarts the handshake */
            }
            send_hello();
        }
        break;
    case BLE_EVENT_TX_UNSUBSCRIBED:
        if (link.connected && event->conn_handle == link.conn_handle) {
            link.subscribed = false;
            protocol_reassembler_reset(&link.rx);
            set_link(LINK_CONNECTED);
        }
        break;
    case BLE_EVENT_PACKET:
        handle_packet(event);
        break;
    }
}

static void process_event(const transport_event_t *event)
{
    if (!event->local_command) {
        handle_event(&event->ble);
    } else if (link.link == LINK_READY
               && event->command.epoch == atomic_load(&ready_epoch)
               && (uint32_t)(now_ms() - event->command.created_ms) <= COMMAND_MAX_AGE_MS) {
        send_message(PROTOCOL_MSG_MEDIA_COMMAND, event->command.payload, 3);
    } else {
        ESP_LOGW(TAG, "Stale button command discarded");
    }
}

static void transport_task(void *arg)
{
    (void)arg;
    static transport_event_t event; /* kept off the task stack */
    uint32_t reported_drops = 0;
    for (;;) {
        if (xQueueReceive(events, &event, portMAX_DELAY) == pdTRUE) {
            process_event(&event);
        }
        const uint32_t drops = atomic_load(&sink_drops);
        if (drops != reported_drops) {
            ESP_LOGW(TAG, "Event queue full: %lu events dropped", (unsigned long)(drops - reported_drops));
            for (uint32_t i = reported_drops; i != drops; ++i) {
                count_error();
            }
            reported_drops = drops;
        }
    }
}

esp_err_t ble_transport_start(void)
{
    ESP_RETURN_ON_FALSE(events == NULL, ESP_ERR_INVALID_STATE, TAG, "Already started");
    unsigned major = 0, minor = 0, patch = 0;
    if (sscanf(esp_app_get_description()->version, "%u.%u.%u", &major, &minor, &patch) != 3) {
        ESP_LOGW(TAG, "App version is not x.y.z; reporting 0.0.0");
        major = minor = patch = 0;
    }
    link.firmware[0] = (uint8_t)major;
    link.firmware[1] = (uint8_t)minor;
    link.firmware[2] = (uint8_t)patch;
    link.link = LINK_BOOT;
    uint8_t *jpeg = NULL;
    if (app_state_artwork_supported()) {
        jpeg = heap_caps_malloc(PROTOCOL_ARTWORK_MAX_BYTES, MALLOC_CAP_SPIRAM);
        artwork_pixels = heap_caps_malloc(ARTWORK_PIXELS * sizeof(uint16_t), MALLOC_CAP_SPIRAM);
        if (jpeg == NULL || artwork_pixels == NULL) {
            ESP_LOGW(TAG, "No PSRAM for artwork transfer; album art disabled");
            heap_caps_free(jpeg);
            heap_caps_free(artwork_pixels);
            jpeg = NULL;
            artwork_pixels = NULL;
        }
    }
    artwork_rx_init(&artwork, jpeg, PROTOCOL_ARTWORK_MAX_BYTES);
    events = xQueueCreate(EVENT_QUEUE_DEPTH, sizeof(transport_event_t));
    ESP_RETURN_ON_FALSE(events != NULL, ESP_ERR_NO_MEM, TAG, "Queue");
    ESP_RETURN_ON_FALSE(xTaskCreate(transport_task, "ble_transport", TASK_STACK, NULL, 5, NULL) == pdPASS,
                        ESP_ERR_NO_MEM, TAG, "Task");
    set_link(LINK_ADVERTISING);
    return ble_service_init(sink);
}

esp_err_t ble_transport_send_command(const protocol_command_t *command)
{
    transport_event_t event = {.local_command = true};
    /* SEEK_DELTA is reserved in V1; the codec validates every other value. */
    if (command == NULL || command->command == PROTOCOL_CMD_SEEK_DELTA
        || protocol_encode_command(command, event.command.payload, 3) != 3) {
        return ESP_ERR_INVALID_ARG;
    }
    event.command.epoch = atomic_load(&ready_epoch);
    event.command.created_ms = now_ms();
    if (events == NULL || event.command.epoch == 0) return ESP_ERR_INVALID_STATE;
    return xQueueSend(events, &event, 0) == pdTRUE ? ESP_OK : ESP_ERR_TIMEOUT;
}
