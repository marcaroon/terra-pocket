#pragma once

#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "esp_lcd_panel_vendor.h"

/* Provisional DevKit N16R8 + 240x240 ST7789 (7-pin, no CS). Confirm module schematic.
 * All wiring and panel-dependent values live here. See docs/hardware.md. */
#define BOARD_LCD_SPI_HOST       SPI2_HOST
#define BOARD_LCD_SCLK           GPIO_NUM_12
#define BOARD_LCD_MOSI           GPIO_NUM_11
/* User module is 7-pin (GND VCC SCL SDA RES DC BLK): CS tied low on the module. */
#define BOARD_LCD_CS             GPIO_NUM_NC
#define BOARD_LCD_DC             GPIO_NUM_9
#define BOARD_LCD_RST            GPIO_NUM_8
#define BOARD_LCD_BL             GPIO_NUM_7
#define BOARD_LCD_BL_ON_LEVEL    1
#define BOARD_LCD_WIDTH          240
#define BOARD_LCD_HEIGHT         240
#define BOARD_LCD_X_GAP          0
/* A square module may need Y gap 80 instead; calibrate for the actual panel. */
#define BOARD_LCD_Y_GAP          0
#define BOARD_LCD_SPI_HZ         (20 * 1000 * 1000)
/* Modules without CS usually need mode 3; try 0 only if the panel stays blank. */
#define BOARD_LCD_SPI_MODE       3
#define BOARD_LCD_BITS_PER_PIXEL 16
#define BOARD_LCD_COLOR_ORDER    LCD_RGB_ELEMENT_ORDER_RGB
#define BOARD_LCD_INVERT_COLOR   true
#define BOARD_LCD_SWAP_XY        false
#define BOARD_LCD_MIRROR_X       false
#define BOARD_LCD_MIRROR_Y       false
#define BOARD_LCD_SWAP_BYTES     true
#define BOARD_LCD_DRAW_LINES     40
/* Period of the LVGL timer that mirrors app_state onto the UI (local timeline). */
#define BOARD_UI_REFRESH_MS      200
#define BOARD_EXPECTED_PSRAM     (8 * 1024 * 1024)

/* Active-low switches to GND; encoder common to GND, contacts pulled to 3V3. */
#define BOARD_BUTTON_PREVIOUS    GPIO_NUM_4
#define BOARD_BUTTON_PLAY        GPIO_NUM_5
#define BOARD_BUTTON_NEXT        GPIO_NUM_6
#define BOARD_ENCODER_A          GPIO_NUM_15
#define BOARD_ENCODER_B          GPIO_NUM_16
#define BOARD_ENCODER_PUSH       GPIO_NUM_17
#define BOARD_INPUT_DEBOUNCE_MS  40
#define BOARD_INPUT_SAMPLE_MS    1
/* Measured 2026-10-11: two transitions per click, detents at AB=11 and AB=00. */
#define BOARD_ENCODER_HALF_STEP  true
/* Reverse this sign if the physical knob logs the opposite direction. */
#define BOARD_ENCODER_DIRECTION  1
#define BOARD_VOLUME_STEP        2
/* Encoder push: shorter is mute (sent on release), this long switches the view. */
#define BOARD_LONG_PRESS_MS      600

/* 1 = boot into the wiring pin scanner (diag/pin_scan.c) instead of the app. */
#define BOARD_DIAG_PIN_SCAN      0
