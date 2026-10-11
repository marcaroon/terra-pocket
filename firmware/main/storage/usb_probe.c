#include "usb_probe.h"
#include "sdkconfig.h"

#if CONFIG_TERRA_USB_PROBE
#if !CONFIG_ESP_CONSOLE_UART_DEFAULT || CONFIG_ESP_CONSOLE_SECONDARY_USB_SERIAL_JTAG
#error "USB probe requires default UART console; native USB PHY belongs to Host"
#endif

#include "disk_inspect.h"
#include <inttypes.h>
#include <stdatomic.h>
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "usb/usb_host.h"
#include "usb/msc_host.h"
/* Driver 1.3.0 deprecates msc_host_read_sector in favor of this API. */
#include "esp_private/msc_scsi_bot.h"

static const char *TAG = "USB_PROBE";
static QueueHandle_t messages;
static atomic_bool queue_fault;

static void msc_event(const msc_host_event_t *event, void *arg)
{
    (void)arg;
    if (event->event != MSC_DEVICE_CONNECTED && event->event != MSC_DEVICE_DISCONNECTED) return;
    if (xQueueSend(messages, event, 0) != pdTRUE) atomic_store(&queue_fault, true);
}

static void host_task(void *arg)
{
    (void)arg;
    for (;;) {
        uint32_t flags;
        ESP_ERROR_CHECK(usb_host_lib_handle_events(portMAX_DELAY, &flags));
        if (flags & USB_HOST_LIB_EVENT_FLAGS_NO_CLIENTS) {
            ESP_ERROR_CHECK(usb_host_device_free_all());
        }
    }
}

static bool read_sector(msc_host_device_handle_t device, uint8_t *buffer,
                        const msc_host_device_info_t *info, uint32_t lba)
{
    if (atomic_load(&queue_fault) || lba >= info->sector_count) return false;
    const esp_err_t err = scsi_cmd_read10(device, buffer, lba, 1, info->sector_size);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Read LBA %" PRIu32 " failed: %s; no further reads", lba, esp_err_to_name(err));
        return false;
    }
    return true;
}

static void inspect_device(msc_host_device_handle_t device)
{
    msc_host_device_info_t info;
    esp_err_t err = msc_host_get_device_info(device, &info);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Device info failed: %s", esp_err_to_name(err));
        return;
    }
    ESP_LOGI(TAG, "VID=%04x PID=%04x sector=%" PRIu32 " count=%" PRIu32 " bytes=%" PRIu64,
             info.idVendor, info.idProduct, info.sector_size, info.sector_count,
             (uint64_t)info.sector_size * info.sector_count);
    if (info.sector_size < 512 || info.sector_size > 4096
        || (info.sector_size & (info.sector_size - 1)) || info.sector_count == 0) {
        ESP_LOGW(TAG, "Unsupported geometry; stopping without mount");
        return;
    }
    uint8_t *buffer = heap_caps_malloc(info.sector_size, MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL);
    if (!buffer) {
        ESP_LOGE(TAG, "No memory for sector buffer");
        return;
    }
    if (!read_sector(device, buffer, &info, 0)) goto done;
    const disk_inspection_t disk = disk_inspect(buffer, info.sector_size, info.sector_count);
    switch (disk.layout) {
    case DISK_NTFS_SIGNATURE:
        ESP_LOGI(TAG, "NTFS signature at LBA 0 (signature only; file access unavailable)");
        break;
    case DISK_GPT:
        ESP_LOGI(TAG, "Protective MBR detected; GPT traversal deferred, no mount");
        break;
    case DISK_MBR:
        ESP_LOGI(TAG, "MBR: %u non-empty primary entries", (unsigned)disk.count);
        for (size_t i = 0; i < disk.count; ++i) {
            const disk_partition_t part = disk.partitions[i];
            ESP_LOGI(TAG, "Partition type=%02x start=%" PRIu32 " sectors=%" PRIu32,
                     part.type, part.start, part.sectors);
            if (!read_sector(device, buffer, &info, part.start)) break;
            const disk_inspection_t volume = disk_inspect(buffer, info.sector_size, part.sectors);
            ESP_LOGI(TAG, "Volume LBA %" PRIu32 ": %s", part.start,
                     volume.layout == DISK_NTFS_SIGNATURE ? "NTFS signature (not mounted)" : "no NTFS signature");
        }
        break;
    case DISK_INVALID:
        ESP_LOGW(TAG, "Invalid/out-of-range metadata; no partition reads");
        break;
    default:
        ESP_LOGI(TAG, "Unknown layout; no partition reads");
        break;
    }
done:
    heap_caps_free(buffer);
    ESP_LOGI(TAG, "Probe finished: no filesystem mounted, no disk writes requested");
}

static void probe_task(void *arg)
{
    (void)arg;
    msc_host_device_handle_t device = NULL;
    for (;;) {
        msc_host_event_t event;
        const bool received = xQueueReceive(messages, &event, pdMS_TO_TICKS(100)) == pdTRUE;
        if (atomic_load(&queue_fault)) {
            /* Lost lifecycle events make handles unreliable. Halt reads until reboot. */
            ESP_LOGE(TAG, "Event queue overflow; probe halted. Restart required.");
            vTaskDelete(NULL);
        }
        if (!received) continue;
        if (event.event == MSC_DEVICE_CONNECTED) {
            if (device) {
                ESP_LOGW(TAG, "Only one disk supported; extra device ignored, reconnect it later");
                continue;
            }
            ESP_LOGI(TAG, "MSC connected: address %u", event.device.address);
            const esp_err_t err = msc_host_install_device(event.device.address, &device);
            if (err != ESP_OK) {
                device = NULL;
                ESP_LOGW(TAG, "Open failed: %s; reconnect to retry", esp_err_to_name(err));
                continue;
            }
            inspect_device(device);
        } else if (device && event.device.handle == device) {
            ESP_ERROR_CHECK(msc_host_uninstall_device(device));
            device = NULL;
            ESP_LOGI(TAG, "Disconnected; waiting for next device");
        }
    }
}

void usb_probe_start(void)
{
    messages = xQueueCreate(8, sizeof(msc_host_event_t));
    ESP_ERROR_CHECK(messages ? ESP_OK : ESP_ERR_NO_MEM);
    const usb_host_config_t host = {.intr_flags = ESP_INTR_FLAG_LOWMED};
    ESP_ERROR_CHECK(usb_host_install(&host));
    ESP_ERROR_CHECK(xTaskCreate(host_task, "usb_host", 4096, NULL, 4, NULL) == pdPASS
                    ? ESP_OK : ESP_ERR_NO_MEM);
    const msc_host_driver_config_t msc = {
        .create_backround_task = true, .task_priority = 5, .stack_size = 4096,
        .core_id = tskNO_AFFINITY, .callback = msc_event,
    };
    ESP_ERROR_CHECK(msc_host_install(&msc));
    ESP_ERROR_CHECK(xTaskCreate(probe_task, "usb_probe", 4096, NULL, 3, NULL) == pdPASS
                    ? ESP_OK : ESP_ERR_NO_MEM);
    ESP_LOGI(TAG, "Read-only sector probe enabled; UART console; verify external HDD power");
}
#endif
