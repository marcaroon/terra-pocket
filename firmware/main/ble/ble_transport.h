#pragma once

/* BLE task: reassembles packets, runs the HELLO handshake and applies validated
 * messages to app_state (docs/ble-protocol.md §5). Never touches LVGL. */
#include "esp_err.h"
#include "protocol/protocol.h"

esp_err_t ble_transport_start(void);
/* Queue a button/encoder command for the current READY session; never wait or replay
 * across a disconnect/re-handshake. ESP_OK means queued, not executed by Windows. */
esp_err_t ble_transport_send_command(const protocol_command_t *command);
