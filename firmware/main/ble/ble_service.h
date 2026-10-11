#pragma once

/* NimBLE GAP/GATT peripheral for the Terra service (docs/ble-protocol.md §1).
 * Knows nothing about the protocol: raw packets and link events are handed to
 * a sink that must only copy/queue them (it runs on the NimBLE host task). */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"

#define BLE_DEVICE_NAME     "Terra"
#define BLE_PACKET_MAX      244   /* preferred ATT MTU 247 - 3 */
#define BLE_DEFAULT_MTU     23

typedef enum {
    BLE_EVENT_CONNECTED,
    BLE_EVENT_DISCONNECTED,
    BLE_EVENT_TX_SUBSCRIBED,
    BLE_EVENT_TX_UNSUBSCRIBED,
    BLE_EVENT_MTU,
    BLE_EVENT_PACKET,
} ble_event_type_t;

typedef struct {
    ble_event_type_t type;
    uint16_t conn_handle;
    uint16_t value;              /* MTU, or packet length for BLE_EVENT_PACKET */
    uint8_t data[BLE_PACKET_MAX];
} ble_event_t;

/* Returns false if the event could not be queued. Link events (`must_deliver`)
 * may wait briefly for queue space; packets never block the host task. */
typedef bool (*ble_event_sink_t)(const ble_event_t *event, bool must_deliver);

esp_err_t ble_service_init(ble_event_sink_t sink);
/* Sends one packet as a TX notification on the given connection. */
esp_err_t ble_service_notify_tx(uint16_t conn_handle, const uint8_t *packet, size_t length);
/* Replaces the DEVICE STATUS value; optionally notifies subscribed clients. */
void ble_service_set_status(const uint8_t *value, size_t length, bool notify);
