#include "display.h"
#include "board_config.h"
#include "ui.h"
#include "esp_check.h"
#include "esp_heap_caps.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_lvgl_port.h"
#include "esp_timer.h"

static const char *TAG = "DISPLAY";
static lv_display_t *display;
static uint16_t *artwork_buffer; /* PSRAM copy that LVGL draws from */
static uint32_t artwork_version;

esp_err_t display_init(void)
{
    const gpio_config_t backlight = {
        .pin_bit_mask = 1ULL << BOARD_LCD_BL,
        .mode = GPIO_MODE_OUTPUT,
    };
    ESP_RETURN_ON_ERROR(gpio_config(&backlight), TAG, "Backlight GPIO");
    ESP_RETURN_ON_ERROR(gpio_set_level(BOARD_LCD_BL, !BOARD_LCD_BL_ON_LEVEL),
                        TAG, "Backlight off");

    const spi_bus_config_t bus = {
        .sclk_io_num = BOARD_LCD_SCLK,
        .mosi_io_num = BOARD_LCD_MOSI,
        .miso_io_num = -1,
        .quadwp_io_num = -1,
        .quadhd_io_num = -1,
        .max_transfer_sz = BOARD_LCD_WIDTH * BOARD_LCD_DRAW_LINES * 2,
    };
    ESP_RETURN_ON_ERROR(spi_bus_initialize(BOARD_LCD_SPI_HOST, &bus, SPI_DMA_CH_AUTO),
                        TAG, "SPI bus");
    const esp_lcd_panel_io_spi_config_t io_config = {
        .cs_gpio_num = BOARD_LCD_CS,
        .dc_gpio_num = BOARD_LCD_DC,
        .spi_mode = BOARD_LCD_SPI_MODE,
        .pclk_hz = BOARD_LCD_SPI_HZ,
        .trans_queue_depth = 10,
        .lcd_cmd_bits = 8,
        .lcd_param_bits = 8,
    };
    esp_lcd_panel_io_handle_t io;
    ESP_RETURN_ON_ERROR(esp_lcd_new_panel_io_spi(
        (esp_lcd_spi_bus_handle_t)BOARD_LCD_SPI_HOST, &io_config, &io), TAG, "LCD IO");
    const esp_lcd_panel_dev_config_t panel_config = {
        .reset_gpio_num = BOARD_LCD_RST,
        .rgb_ele_order = BOARD_LCD_COLOR_ORDER,
        .bits_per_pixel = BOARD_LCD_BITS_PER_PIXEL,
    };
    esp_lcd_panel_handle_t panel;
    ESP_RETURN_ON_ERROR(esp_lcd_new_panel_st7789(io, &panel_config, &panel), TAG, "ST7789");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_reset(panel), TAG, "Reset");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_init(panel), TAG, "Panel init");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_invert_color(panel, BOARD_LCD_INVERT_COLOR), TAG, "Inversion");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_set_gap(panel, BOARD_LCD_X_GAP, BOARD_LCD_Y_GAP), TAG, "Offset");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_disp_on_off(panel, true), TAG, "Panel on");
    ESP_LOGI(TAG, "ST7789 initialized (%dx%d, gap %d/%d)",
             BOARD_LCD_WIDTH, BOARD_LCD_HEIGHT, BOARD_LCD_X_GAP, BOARD_LCD_Y_GAP);

    const lvgl_port_cfg_t lvgl_config = ESP_LVGL_PORT_INIT_CONFIG();
    ESP_RETURN_ON_ERROR(lvgl_port_init(&lvgl_config), TAG, "LVGL init");
    const lvgl_port_display_cfg_t display_config = {
        .io_handle = io,
        .panel_handle = panel,
        .buffer_size = BOARD_LCD_WIDTH * BOARD_LCD_DRAW_LINES,
        .double_buffer = true,
        .hres = BOARD_LCD_WIDTH,
        .vres = BOARD_LCD_HEIGHT,
        .color_format = LV_COLOR_FORMAT_RGB565,
        .rotation = {
            .swap_xy = BOARD_LCD_SWAP_XY,
            .mirror_x = BOARD_LCD_MIRROR_X,
            .mirror_y = BOARD_LCD_MIRROR_Y,
        },
        .flags = {.buff_dma = true, .swap_bytes = BOARD_LCD_SWAP_BYTES},
    };
    display = lvgl_port_add_disp(&display_config);
    ESP_RETURN_ON_FALSE(display != NULL, ESP_ERR_NO_MEM, TAG, "Display registration");
    ESP_LOGI("LVGL", "Display registered");
    return ESP_OK;
}

static uint32_t now_ms(void)
{
    return (uint32_t)(esp_timer_get_time() / 1000);
}

/* Runs inside the LVGL task with the LVGL lock held. Reads a copy of app_state;
 * BLE never touches widgets (spec section 7). */
static void refresh(lv_timer_t *timer)
{
    (void)timer;
    media_state_t snapshot;
    app_state_snapshot(&snapshot);
    if (artwork_buffer != NULL && snapshot.artwork_version != artwork_version) {
        /* Copied here, in the LVGL task, so the buffer never changes mid-draw. */
        artwork_version = snapshot.artwork_version;
        snapshot.artwork = app_state_copy_artwork(artwork_buffer);
    }
    ui_update(&snapshot, now_ms());
}

esp_err_t display_start_ui(void)
{
    ESP_RETURN_ON_FALSE(display != NULL, ESP_ERR_INVALID_STATE, TAG, "Display not initialized");
    ESP_RETURN_ON_FALSE(lvgl_port_lock(1000), ESP_ERR_TIMEOUT, TAG, "LVGL lock");
    if (app_state_artwork_supported()) {
        artwork_buffer = heap_caps_malloc(PROTOCOL_ARTWORK_SIDE * PROTOCOL_ARTWORK_SIDE * sizeof(uint16_t),
                                          MALLOC_CAP_SPIRAM);
    }
    ui_create(lv_display_get_screen_active(display), artwork_buffer);
    refresh(NULL);
    /* Flush before enabling the backlight, avoiding an uninitialized frame. */
    lv_refr_now(display);
    lv_timer_t *timer = lv_timer_create(refresh, BOARD_UI_REFRESH_MS, NULL);
    lvgl_port_unlock();
    ESP_RETURN_ON_FALSE(timer != NULL, ESP_ERR_NO_MEM, TAG, "UI timer");
    return gpio_set_level(BOARD_LCD_BL, BOARD_LCD_BL_ON_LEVEL);
}
