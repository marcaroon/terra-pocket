#pragma once

/* Wiring diagnostic: classify header GPIOs, then log every level change.
 * Never returns. Enabled with BOARD_DIAG_PIN_SCAN in board_config.h. */
void pin_scan_run(void);
