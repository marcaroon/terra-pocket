#include "ble/ble_service.h"
#include <string.h>
#include "esp_check.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "host/ble_hs.h"
#include "host/util/util.h"
#include "nimble/nimble_port.h"
#include "nimble/nimble_port_freertos.h"
#include "services/gap/ble_svc_gap.h"
#include "services/gatt/ble_svc_gatt.h"

static const char *TAG = "BLE";

/* 6F3A00xx-9B21-4C71-9A8B-31A8B9F10001, little-endian byte order. */
#define TERRA_UUID(id) BLE_UUID128_INIT(0x01, 0x00, 0xF1, 0xB9, 0xA8, 0x31, 0x8B, 0x9A, \
                                         0x71, 0x4C, 0x21, 0x9B, (id), 0x00, 0x3A, 0x6F)
static const ble_uuid128_t service_uuid = TERRA_UUID(0x01);
static const ble_uuid128_t rx_uuid = TERRA_UUID(0x02);
static const ble_uuid128_t tx_uuid = TERRA_UUID(0x03);
static const ble_uuid128_t status_uuid = TERRA_UUID(0x04);

static ble_event_sink_t sink;
static uint8_t own_addr_type;
static uint16_t tx_handle;
static uint16_t status_handle;
static portMUX_TYPE status_lock = portMUX_INITIALIZER_UNLOCKED;
static uint8_t status_value[16];
static size_t status_length;

static void emit(ble_event_type_t type, uint16_t conn_handle, uint16_t value)
{
    const ble_event_t event = {.type = type, .conn_handle = conn_handle, .value = value};
    if (!sink(&event, true)) {
        ESP_LOGW(TAG, "Event queue full; link event %d lost", type);
    }
}

static int access_cb(uint16_t conn_handle, uint16_t attr_handle,
                     struct ble_gatt_access_ctxt *ctxt, void *arg)
{
    (void)attr_handle;
    (void)arg;
    if (ctxt->op == BLE_GATT_ACCESS_OP_WRITE_CHR && ble_uuid_cmp(ctxt->chr->uuid, &rx_uuid.u) == 0) {
        const uint16_t length = OS_MBUF_PKTLEN(ctxt->om);
        if (length > BLE_PACKET_MAX) {
            return BLE_ATT_ERR_INVALID_ATTR_VALUE_LEN;
        }
        ble_event_t event = {.type = BLE_EVENT_PACKET, .conn_handle = conn_handle};
        uint16_t copied = 0;
        if (ble_hs_mbuf_to_flat(ctxt->om, event.data, sizeof(event.data), &copied) != 0) {
            return BLE_ATT_ERR_UNLIKELY;
        }
        event.value = copied;
        return sink(&event, false) ? 0 : BLE_ATT_ERR_INSUFFICIENT_RES;
    }
    if (ctxt->op == BLE_GATT_ACCESS_OP_READ_CHR && ble_uuid_cmp(ctxt->chr->uuid, &status_uuid.u) == 0) {
        uint8_t copy[sizeof(status_value)];
        taskENTER_CRITICAL(&status_lock);
        const size_t length = status_length;
        memcpy(copy, status_value, length);
        taskEXIT_CRITICAL(&status_lock);
        return os_mbuf_append(ctxt->om, copy, length) == 0 ? 0 : BLE_ATT_ERR_INSUFFICIENT_RES;
    }
    return BLE_ATT_ERR_UNLIKELY;
}

static const struct ble_gatt_svc_def services[] = {
    {
        .type = BLE_GATT_SVC_TYPE_PRIMARY,
        .uuid = &service_uuid.u,
        .characteristics = (struct ble_gatt_chr_def[]) {
            {
                .uuid = &rx_uuid.u,
                .access_cb = access_cb,
                .flags = BLE_GATT_CHR_F_WRITE | BLE_GATT_CHR_F_WRITE_NO_RSP,
            },
            {
                .uuid = &tx_uuid.u,
                .access_cb = access_cb,
                .flags = BLE_GATT_CHR_F_NOTIFY,
                .val_handle = &tx_handle,
            },
            {
                .uuid = &status_uuid.u,
                .access_cb = access_cb,
                .flags = BLE_GATT_CHR_F_READ | BLE_GATT_CHR_F_NOTIFY,
                .val_handle = &status_handle,
            },
            {0},
        },
    },
    {0},
};

static int gap_event(struct ble_gap_event *event, void *arg);

static void advertise(void)
{
    struct ble_hs_adv_fields fields = {0};
    fields.flags = BLE_HS_ADV_F_DISC_GEN | BLE_HS_ADV_F_BREDR_UNSUP;
    const char *name = ble_svc_gap_device_name();
    fields.name = (const uint8_t *)name;
    fields.name_len = (uint8_t)strlen(name);
    fields.name_is_complete = 1;
    int rc = ble_gap_adv_set_fields(&fields);
    if (rc != 0) {
        ESP_LOGE(TAG, "Advertising data rejected (rc=%d)", rc);
        return;
    }
    struct ble_hs_adv_fields response = {0};
    response.uuids128 = &service_uuid;
    response.num_uuids128 = 1;
    response.uuids128_is_complete = 1;
    rc = ble_gap_adv_rsp_set_fields(&response);
    if (rc != 0) {
        ESP_LOGE(TAG, "Scan response rejected (rc=%d)", rc);
        return;
    }
    const struct ble_gap_adv_params params = {
        .conn_mode = BLE_GAP_CONN_MODE_UND,
        .disc_mode = BLE_GAP_DISC_MODE_GEN,
    };
    rc = ble_gap_adv_start(own_addr_type, NULL, BLE_HS_FOREVER, &params, gap_event, NULL);
    if (rc != 0 && rc != BLE_HS_EALREADY) {
        ESP_LOGE(TAG, "Advertising start failed (rc=%d)", rc);
        return;
    }
    ESP_LOGI(TAG, "Advertising as %s", name);
}

static int gap_event(struct ble_gap_event *event, void *arg)
{
    (void)arg;
    switch (event->type) {
    case BLE_GAP_EVENT_CONNECT:
        if (event->connect.status == 0) {
            ESP_LOGI(TAG, "Client connected");
            emit(BLE_EVENT_CONNECTED, event->connect.conn_handle, BLE_DEFAULT_MTU);
        } else {
            ESP_LOGW(TAG, "Connection failed (status=%d)", event->connect.status);
            advertise();
        }
        break;
    case BLE_GAP_EVENT_DISCONNECT:
        ESP_LOGI(TAG, "Client disconnected (reason=0x%03x)", event->disconnect.reason);
        emit(BLE_EVENT_DISCONNECTED, event->disconnect.conn.conn_handle, 0);
        advertise();
        break;
    case BLE_GAP_EVENT_SUBSCRIBE:
        if (event->subscribe.attr_handle == tx_handle) {
            emit(event->subscribe.cur_notify ? BLE_EVENT_TX_SUBSCRIBED : BLE_EVENT_TX_UNSUBSCRIBED,
                 event->subscribe.conn_handle, 0);
        }
        break;
    case BLE_GAP_EVENT_MTU:
        ESP_LOGI(TAG, "MTU %u", event->mtu.value);
        emit(BLE_EVENT_MTU, event->mtu.conn_handle, event->mtu.value);
        break;
    case BLE_GAP_EVENT_ADV_COMPLETE:
        advertise();
        break;
    default:
        break;
    }
    return 0;
}

static void on_sync(void)
{
    int rc = ble_hs_util_ensure_addr(0);
    if (rc == 0) {
        rc = ble_hs_id_infer_auto(0, &own_addr_type);
    }
    if (rc != 0) {
        ESP_LOGE(TAG, "No usable BLE address (rc=%d)", rc);
        return;
    }
    advertise();
}

static void on_reset(int reason)
{
    ESP_LOGW(TAG, "Host reset (reason=%d)", reason);
}

static void host_task(void *param)
{
    (void)param;
    nimble_port_run();
    nimble_port_freertos_deinit();
}

esp_err_t ble_service_init(ble_event_sink_t event_sink)
{
    ESP_RETURN_ON_FALSE(event_sink != NULL && sink == NULL, ESP_ERR_INVALID_STATE, TAG, "Sink");
    sink = event_sink;
    ESP_RETURN_ON_ERROR(nimble_port_init(), TAG, "NimBLE init");
    ble_hs_cfg.sync_cb = on_sync;
    ble_hs_cfg.reset_cb = on_reset;
    ble_svc_gap_init();
    ble_svc_gatt_init();
    ESP_RETURN_ON_FALSE(ble_gatts_count_cfg(services) == 0, ESP_FAIL, TAG, "GATT count");
    ESP_RETURN_ON_FALSE(ble_gatts_add_svcs(services) == 0, ESP_FAIL, TAG, "GATT add");
    ESP_RETURN_ON_FALSE(ble_svc_gap_device_name_set(BLE_DEVICE_NAME) == 0, ESP_FAIL, TAG, "Name");
    nimble_port_freertos_init(host_task);
    return ESP_OK;
}

esp_err_t ble_service_notify_tx(uint16_t conn_handle, const uint8_t *packet, size_t length)
{
    if (packet == NULL || length == 0 || length > BLE_PACKET_MAX) {
        return ESP_ERR_INVALID_ARG;
    }
    struct os_mbuf *om = ble_hs_mbuf_from_flat(packet, (uint16_t)length);
    if (om == NULL) {
        return ESP_ERR_NO_MEM;
    }
    /* Consumes the mbuf regardless of the result. */
    return ble_gatts_notify_custom(conn_handle, tx_handle, om) == 0 ? ESP_OK : ESP_FAIL;
}

void ble_service_set_status(const uint8_t *value, size_t length, bool notify)
{
    if (value == NULL || length > sizeof(status_value)) {
        return;
    }
    taskENTER_CRITICAL(&status_lock);
    memcpy(status_value, value, length);
    status_length = length;
    taskEXIT_CRITICAL(&status_lock);
    if (notify && status_handle != 0) {
        ble_gatts_chr_updated(status_handle);
    }
}
