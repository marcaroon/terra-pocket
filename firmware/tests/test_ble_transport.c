/* Exercise the production transport with a fake scheduler/radio, not fake protocol logic. */
#include <assert.h>
#include <stdio.h>
#include "../main/ble/ble_transport.c"

static transport_event_t queue[EVENT_QUEUE_DEPTH];
static unsigned head, tail, queued, notifications;
static uint32_t clock_ms;
static uint8_t last_packet[BLE_PACKET_MAX];
static size_t last_length;

const esp_app_desc_t *esp_app_get_description(void) {
    static const esp_app_desc_t desc = {.version = "0.3.0"};
    return &desc;
}
int64_t esp_timer_get_time(void) { return (int64_t)clock_ms * 1000; }
QueueHandle_t xQueueCreate(unsigned depth, size_t size) {
    assert(depth == EVENT_QUEUE_DEPTH && size == sizeof(transport_event_t));
    return queue;
}
int xQueueSend(QueueHandle_t q, const void *item, TickType_t wait) {
    (void)wait; assert(q == queue);
    if (queued == EVENT_QUEUE_DEPTH) return 0;
    queue[tail++ % EVENT_QUEUE_DEPTH] = *(const transport_event_t *)item;
    ++queued; return pdTRUE;
}
int xQueueReceive(QueueHandle_t q, void *item, TickType_t wait) {
    (void)wait; assert(q == queue);
    if (!queued) return 0;
    *(transport_event_t *)item = queue[head++ % EVENT_QUEUE_DEPTH];
    --queued; return pdTRUE;
}
int xTaskCreate(void (*task)(void *), const char *name, unsigned stack, void *arg, unsigned priority, void *handle) {
    (void)task; (void)name; (void)stack; (void)arg; (void)priority; (void)handle;
    return pdPASS;
}
void vTaskDelay(TickType_t ticks) { clock_ms += ticks; }
esp_err_t ble_service_init(ble_event_sink_t callback) { assert(callback == sink); return ESP_OK; }
esp_err_t ble_service_notify_tx(uint16_t connection, const uint8_t *packet, size_t length) {
    assert(connection == 7); assert(length <= sizeof(last_packet));
    memcpy(last_packet, packet, length); last_length = length; ++notifications;
    return ESP_OK;
}
void ble_service_set_status(const uint8_t *value, size_t length, bool notify) {
    (void)notify; protocol_status_t status;
    assert(protocol_decode_status(value, length, &status));
}
void app_state_set_link(link_state_t state) { (void)state; }
void app_state_snapshot(media_state_t *out) { memset(out, 0, sizeof(*out)); }
void app_state_apply_media(const protocol_media_state_t *media, uint32_t now) { (void)media; (void)now; }
void app_state_apply_volume(const protocol_volume_t *volume) { (void)volume; }

static void drain(void) {
    transport_event_t event;
    while (xQueueReceive(events, &event, 0)) process_event(&event);
}
static void emit_event(ble_event_type_t type) {
    const ble_event_t event = {.type = type, .conn_handle = 7};
    assert(sink(&event, true)); drain();
}
static void ack(void) {
    const uint8_t payload[] = {1, 0, 1, 0, 0};
    ble_event_t packet = {.type = BLE_EVENT_PACKET, .conn_handle = 7};
    packet.value = (uint16_t)protocol_fragment_build(PROTOCOL_MSG_HELLO_ACK, 0, payload,
        sizeof(payload), 23, 0, packet.data, sizeof(packet.data));
    assert(sink(&packet, false)); drain();
}

int main(void) {
    assert(ble_transport_start() == ESP_OK);
    protocol_command_t command = {PROTOCOL_CMD_PLAY_PAUSE, 0};
    assert(ble_transport_send_command(&command) == ESP_ERR_INVALID_STATE);
    emit_event(BLE_EVENT_CONNECTED);
    ack(); /* Cannot become READY before subscribing. */
    assert(link.link == LINK_CONNECTED);
    emit_event(BLE_EVENT_TX_SUBSCRIBED);
    assert(last_packet[1] == PROTOCOL_MSG_HELLO);
    protocol_hello_t hello;
    assert(protocol_decode_hello(last_packet + 4, last_length - 4, &hello));
    assert(hello.capabilities & PROTOCOL_CAP_MEDIA_COMMAND);
    ack(); assert(link.link == LINK_READY);
    const unsigned before_duplicate = notifications;
    ack();
    assert(link.link == LINK_READY && notifications == before_duplicate + 1);
    assert(last_packet[1] == PROTOCOL_MSG_ERROR && last_packet[4] == PROTOCOL_ERR_NOT_READY);
    for (uint8_t id = PROTOCOL_CMD_PLAY_PAUSE; id <= PROTOCOL_CMD_PREVIOUS; ++id) {
        command.command = id;
        const unsigned before = notifications;
        assert(ble_transport_send_command(&command) == ESP_OK); drain();
        assert(notifications == before + 1);
        assert(last_length == 7 && last_packet[1] == PROTOCOL_MSG_MEDIA_COMMAND);
        protocol_command_t decoded;
        assert(protocol_decode_command(last_packet + 4, 3, &decoded));
        assert(decoded.command == id && decoded.value == 0);
    }
    /* M8 encoder: volume steps and mute toggle reach the companion unchanged. */
    const protocol_command_t encoder[] = {
        {PROTOCOL_CMD_VOLUME_DELTA, 2}, {PROTOCOL_CMD_VOLUME_DELTA, -2}, {PROTOCOL_CMD_MUTE_TOGGLE, 0},
    };
    for (size_t i = 0; i < sizeof(encoder) / sizeof(encoder[0]); ++i) {
        const unsigned before = notifications;
        assert(ble_transport_send_command(&encoder[i]) == ESP_OK); drain();
        assert(notifications == before + 1 && last_packet[1] == PROTOCOL_MSG_MEDIA_COMMAND);
        protocol_command_t decoded;
        assert(protocol_decode_command(last_packet + 4, 3, &decoded));
        assert(decoded.command == encoder[i].command && decoded.value == encoder[i].value);
    }
    const protocol_command_t invalid[] = {
        {PROTOCOL_CMD_VOLUME_DELTA, 0}, {PROTOCOL_CMD_VOLUME_DELTA, 101},
        {PROTOCOL_CMD_MUTE_TOGGLE, 1}, {PROTOCOL_CMD_SEEK_DELTA, 5}, {0x07, 0},
    };
    for (size_t i = 0; i < sizeof(invalid) / sizeof(invalid[0]); ++i) {
        assert(ble_transport_send_command(&invalid[i]) == ESP_ERR_INVALID_ARG);
    }
    assert(ble_transport_send_command(&command) == ESP_OK);
    const unsigned before_stale = notifications;
    clock_ms += COMMAND_MAX_AGE_MS + 1; drain();
    assert(notifications == before_stale);
    assert(ble_transport_send_command(&command) == ESP_OK);
    /* A lifecycle callback invalidates commands already queued ahead of it. */
    const ble_event_t disconnected = {.type = BLE_EVENT_DISCONNECTED, .conn_handle = 7};
    assert(sink(&disconnected, true)); drain();
    assert(notifications == before_stale && link.link == LINK_ADVERTISING);
    assert(ble_transport_send_command(&command) == ESP_ERR_INVALID_STATE);
    emit_event(BLE_EVENT_CONNECTED); emit_event(BLE_EVENT_TX_SUBSCRIBED); ack();
    transport_event_t old = {.local_command = true, .command = {.epoch = 1, .created_ms = clock_ms}};
    const unsigned before_old = notifications;
    process_event(&old); assert(notifications == before_old);
    command.value = 1;
    assert(ble_transport_send_command(&command) == ESP_ERR_INVALID_ARG);
    command.value = 0;
    for (unsigned i = 0; i < EVENT_QUEUE_DEPTH; ++i) assert(ble_transport_send_command(&command) == ESP_OK);
    assert(ble_transport_send_command(&command) == ESP_ERR_TIMEOUT);
    drain();
    link.rx.active = true; /* A partial message must not survive unsubscribe. */
    emit_event(BLE_EVENT_TX_UNSUBSCRIBED);
    assert(!link.rx.active && atomic_load(&ready_epoch) == 0);
    emit_event(BLE_EVENT_TX_SUBSCRIBED); ack();
    assert(link.link == LINK_READY);
    assert(ble_transport_send_command(&command) == ESP_OK);
    const unsigned before_resubscribe = notifications;
    const ble_event_t resubscribe = {.type = BLE_EVENT_TX_SUBSCRIBED, .conn_handle = 7};
    assert(sink(&resubscribe, true)); drain();
    /* Only HELLO, never the command queued before re-subscription. */
    assert(notifications == before_resubscribe + 1 && last_packet[1] == PROTOCOL_MSG_HELLO);
    puts("PASS BLE transport: handshake, buttons, volume/mute, offline/stale/old-session, queue overflow");
}
