#pragma once

/* BLE task: reassembles packets, runs the HELLO handshake and applies validated
 * messages to app_state (docs/ble-protocol.md §5). Never touches LVGL. */
#include "esp_err.h"

esp_err_t ble_transport_start(void);
