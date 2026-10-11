#include "ble/ble_transport.h"
#include <stdio.h>
#include <string.h>
#include "app_state.h"
#include "ble/ble_service.h"
#include "esp_app_desc.h"
#include "esp_check.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "protocol/protocol.h"

static const char *TAG = "PROTOCOL";

#define EVENT_QUEUE_DEPTH  16
#define NOTIFY_RETRIES     5
/* M3/M4 render media and volume; MEDIA_COMMAND is added with M7. */
#define CAPABILITIES       (PROTOCOL_CAP_MEDIA_STATE | PROTOCOL_CAP_VOLUME_STATE)

static QueueHandle_t events;
static volatile uint32_t sink_drops;

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
    if (xQueueSend(events, event, must_deliver ? pdMS_TO_TICKS(50) : 0) != pdTRUE) {
        ++sink_drops;
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
        .capabilities = CAPABILITIES,
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

static void handle_message(uint8_t type, uint8_t seq, const uint8_t *payload, size_t length)
{
    switch (type) {
    case PROTOCOL_MSG_HELLO_ACK: {
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
            set_link(LINK_CONNECTED);
        }
        break;
    case BLE_EVENT_PACKET:
        handle_packet(event);
        break;
    }
}

static void transport_task(void *arg)
{
    (void)arg;
    static ble_event_t event; /* 250 bytes; kept off the task stack */
    uint32_t reported_drops = 0;
    for (;;) {
        if (xQueueReceive(events, &event, portMAX_DELAY) == pdTRUE) {
            handle_event(&event);
        }
        const uint32_t drops = sink_drops;
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
    events = xQueueCreate(EVENT_QUEUE_DEPTH, sizeof(ble_event_t));
    ESP_RETURN_ON_FALSE(events != NULL, ESP_ERR_NO_MEM, TAG, "Queue");
    ESP_RETURN_ON_FALSE(xTaskCreate(transport_task, "ble_transport", 4096, NULL, 5, NULL) == pdPASS,
                        ESP_ERR_NO_MEM, TAG, "Task");
    set_link(LINK_ADVERTISING);
    return ble_service_init(sink);
}
