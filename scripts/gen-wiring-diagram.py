"""Generate docs/wiring-diagram.svg for Terra (mode N0 + M2 wiring)."""
import sys

W, H = 1240, 1090
BX0, BX1 = 500, 760          # board edges (J1 left, J3 right)
PY0, PSTEP = 140, 36          # pin 1 y, pitch

J1 = ["3V3", "3V3", "RST", "GPIO4", "GPIO5", "GPIO6", "GPIO7", "GPIO15", "GPIO16",
      "GPIO17", "GPIO18", "GPIO8", "GPIO3", "GPIO46", "GPIO9", "GPIO10", "GPIO11",
      "GPIO12", "GPIO13", "GPIO14", "5V", "GND"]
J3 = ["GND", "TX/43", "RX/44", "GPIO1", "GPIO2", "GPIO42", "GPIO41", "GPIO40",
      "GPIO39", "GPIO38", "GPIO37", "GPIO36", "GPIO35", "GPIO0", "GPIO45", "GPIO48",
      "GPIO47", "GPIO21", "GPIO20", "GPIO19", "GND", "GND"]
USED_J1 = {1, 4, 5, 6, 7, 8, 9, 10, 12, 15, 17, 18, 22}
USED_J3 = {19, 20, 21}
AVOID = {"GPIO0", "GPIO3", "GPIO45", "GPIO46", "GPIO35", "GPIO36", "GPIO37"}

C = dict(pwr="#d93025", gnd="#202124", spi="#1a73e8", btn="#188038", enc="#e37400",
         usb="#9334e6", bl="#c5221f", muted="#9aa0a6", ink="#202124", sub="#5f6368",
         board="#e8f0fe", boardline="#1a73e8", box="#ffffff", boxline="#5f6368",
         warn="#b06000")

out = []
a = out.append


def py(n):
    return PY0 + (n - 1) * PSTEP


def text(x, y, s, size=13, anchor="start", color=None, weight="normal", italic=False):
    st = ' font-style="italic"' if italic else ""
    a(f'<text x="{x}" y="{y}" font-size="{size}" text-anchor="{anchor}" '
      f'fill="{color or C["ink"]}" font-weight="{weight}"{st}>{s}</text>')


def line(pts, color, w=2.5, dash=None):
    d = " ".join(f"{x},{y}" for x, y in pts)
    ds = f' stroke-dasharray="{dash}"' if dash else ""
    a(f'<polyline points="{d}" fill="none" stroke="{color}" stroke-width="{w}" '
      f'stroke-linejoin="round" stroke-linecap="round"{ds}/>')


def dot(x, y, color):
    a(f'<circle cx="{x}" cy="{y}" r="4" fill="{color}"/>')


def box(x, y, w, h, title, sub=None, fill=None, stroke=None):
    a(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="{fill or C["box"]}" '
      f'stroke="{stroke or C["boxline"]}" stroke-width="1.5"/>')
    text(x + w / 2, y + 20, title, 14, "middle", weight="bold")
    if sub:
        text(x + w / 2, y + 37, sub, 11, "middle", C["sub"])


def gnd(x, y):
    """Ground symbol whose top touches (x, y)."""
    line([(x, y), (x, y + 8)], C["gnd"], 2)
    for i, half in enumerate((10, 6, 2)):
        yy = y + 8 + i * 4
        line([(x - half, yy), (x + half, yy)], C["gnd"], 2)


def switch(x0, x1, y, color):
    """Push-button symbol on a horizontal wire between x0 (right) and x1 (left)."""
    mid = (x0 + x1) / 2
    line([(x0, y), (mid + 14, y)], color)
    line([(mid - 14, y), (x1, y)], color)
    dot(mid + 14, y, color)
    dot(mid - 14, y, color)
    line([(mid - 16, y - 10), (mid + 16, y - 10)], color, 2)
    line([(mid, y - 10), (mid, y - 18)], color, 2)


def flag(x, y, label, color, left=True):
    """Net label flag; wire end at (x, y)."""
    wdt = 9 + 7.2 * len(label)
    if left:
        pts = f"{x},{y} {x-10},{y-10} {x-10-wdt},{y-10} {x-10-wdt},{y+10} {x-10},{y+10}"
        tx = x - 10 - wdt / 2
    else:
        pts = f"{x},{y} {x+10},{y-10} {x+10+wdt},{y-10} {x+10+wdt},{y+10} {x+10},{y+10}"
        tx = x + 10 + wdt / 2
    a(f'<polygon points="{pts}" fill="#fff" stroke="{color}" stroke-width="1.5"/>')
    text(tx, y + 4, label, 11, "middle", color, "bold")


a(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" '
  'font-family="Segoe UI, Roboto, Helvetica, Arial, sans-serif" role="img" '
  'aria-labelledby="t d">')
a('<title id="t">Terra — diagram wiring hardware</title>')
a('<desc id="d">ESP32-S3 N16R8 ke ST7789 240x240, tiga tombol, rotary encoder, '
  'dan HDD USB melalui GPIO19/20 dengan VBUS dari adaptor 5 V terpisah.</desc>')
a(f'<rect width="{W}" height="{H}" fill="#ffffff"/>')

text(24, 36, "Terra — wiring hardware (M2 input + N0 USB probe)", 20, weight="bold")
text(24, 58, "Diagram logis, bukan layout PCB. Nomor header mengikuti pinout bergaya "
     "DevKitC-1; cocokkan dengan silkscreen board YD sebelum menyolder.", 12, color=C["sub"])

# ---------------- board ----------------
a(f'<rect x="{BX0}" y="{PY0-50}" width="{BX1-BX0}" height="{py(22)-PY0+100}" rx="10" '
  f'fill="{C["board"]}" stroke="{C["boardline"]}" stroke-width="2"/>')
text((BX0 + BX1) / 2, PY0 - 26, "ESP32-S3 N16R8", 15, "middle", weight="bold")
text((BX0 + BX1) / 2, PY0 - 10, "YD-ESP32-S3 2022-V1.3 (tampak atas)", 11, "middle", C["sub"])
text(BX0 + 8, PY0 - 34, "J1", 12, weight="bold", color=C["boardline"])
text(BX1 - 8, PY0 - 34, "J3", 12, "end", weight="bold", color=C["boardline"])
for i, name in enumerate(J1, 1):
    y = py(i)
    used = i in USED_J1
    col = C["ink"] if used else (C["warn"] if name in AVOID else C["muted"])
    a(f'<circle cx="{BX0}" cy="{y}" r="6" fill="{"#fff" if not used else "#fbbc04"}" '
      f'stroke="{C["boardline"]}" stroke-width="1.5"/>')
    text(BX0 + 12, y + 4, f"{i:>2} {name}", 12, color=col, weight="bold" if used else "normal")
for i, name in enumerate(J3, 1):
    y = py(i)
    used = i in USED_J3
    col = C["ink"] if used else (C["warn"] if name in AVOID else C["muted"])
    a(f'<circle cx="{BX1}" cy="{y}" r="6" fill="{"#fff" if not used else "#fbbc04"}" '
      f'stroke="{C["boardline"]}" stroke-width="1.5"/>')
    text(BX1 - 12, y + 4, f"{name} {i:>2}", 12, "end", col, "bold" if used else "normal")
# USB connectors at the bottom edge
by = py(22) + 50
for cx, lab, sub in ((BX0 + 70, "COM", "UART bridge"), (BX1 - 70, "USB", "native, GPIO19/20")):
    a(f'<rect x="{cx-30}" y="{by-14}" width="60" height="22" rx="6" fill="#dadce0" '
      f'stroke="{C["boxline"]}"/>')
    text(cx, by + 2, lab, 11, "middle", weight="bold")
    text(cx, by + 24, sub, 10, "middle", C["sub"])
text((BX0 + BX1) / 2, py(22) + 30, "label konektor: ikuti PCB", 10, "middle", C["sub"], italic=True)

# ---------------- inputs (left, straight wires) ----------------
box(60, py(4) - 44, 300, py(6) - py(4) + 74, "Tombol tactile 6×6 mm")
for n, lab in ((4, "SW1 Previous"), (5, "SW2 Play/Pause"), (6, "SW3 Next")):
    y = py(n)
    line([(BX0, y), (330, y)], C["btn"])
    switch(330, 160, y, C["btn"])
    text(150, y + 4, lab, 12, "end")
# shared ground of buttons
line([(160, py(4)), (160, py(6) + 12)], C["gnd"], 2)
gnd(160, py(6) + 12)
for n in (4, 5, 6):
    dot(160, py(n), C["gnd"])

# GPIO7 -> LCD BLK via net flag
y7 = py(7)
line([(BX0, y7), (470, y7)], C["bl"])
flag(470, y7, "LCD BLK", C["bl"])

box(60, py(8) - 38, 300, py(10) - py(8) + 64, "Rotary encoder EC11 + push")
enc_pins = ((8, "A / CLK"), (9, "B / DT"), (10, "SW"))
for n, lab in enc_pins:
    y = py(n)
    line([(BX0, y), (300, y)], C["enc"])
    a(f'<circle cx="300" cy="{y}" r="4" fill="{C["enc"]}"/>')
    text(290, y + 4, lab, 12, "end")
text(110, py(9) + 4, "C (tengah) → GND", 11, "start", C["sub"])
text(110, py(10) + 4, "SW' → GND", 11, "start", C["sub"])
line([(100, py(9)), (90, py(9)), (90, py(10) + 6)], C["gnd"], 2)
line([(100, py(10)), (90, py(10))], C["gnd"], 2)
gnd(90, py(10) + 6)

# ---------------- LCD (left bottom, straight wires) ----------------
lcd_top = py(12) - 40
box(60, lcd_top, 300, py(18) - lcd_top + 40, "LCD ST7789 240×240 · SPI mode 3")
for n, lab in ((12, "RES / RST"), (15, "DC / A0"), (17, "SDA / DIN / MOSI"),
               (18, "SCL / SCK / CLK")):
    y = py(n)
    line([(BX0, y), (300, y)], C["spi"])
    a(f'<circle cx="300" cy="{y}" r="4" fill="{C["spi"]}"/>')
    text(290, y + 4, lab, 12, "end")
text(290, py(16) + 4, "tanpa CS (7 pin) · GPIO10 kosong", 11, "end", C["warn"], italic=True)
# LCD power pins, net flags on the left side of the box
for k, (lab, net, col) in enumerate((("VCC", "3V3", C["pwr"]), ("GND", "GND", C["gnd"]),
                                     ("BLK", "GPIO7", C["bl"]))):
    y = py(13) + k * 32
    text(110, y + 4, lab, 12)
    line([(100, y), (80, y)], col)
    flag(80, y, net, col)
# J1 3V3 / 5V / GND
line([(BX0, py(1)), (430, py(1))], C["pwr"])
flag(430, py(1), "3V3 → LCD VCC", C["pwr"])
line([(BX0, py(21)), (470, py(21))], C["muted"], 2, "5 4")
text(462, py(21) + 4, "5V: kosong pada M2/N0", 11, "end", C["sub"], italic=True)
line([(BX0, py(22)), (440, py(22))], C["gnd"])
flag(440, py(22), "GND (rail bersama)", C["gnd"])

# ---------------- USB host side (right) ----------------
ux0, ux1 = 880, 1040
box(ux0, py(17) - 40, ux1 - ux0, py(22) - py(17) + 64, "USB-A female", "breakout, ke kabel casing")
usb_pins = ((19, "D+", C["usb"]), (20, "D−", C["usb"]), (21, "GND", C["gnd"]))
for n, lab, col in usb_pins:
    y = py(n)
    line([(BX1, y), (ux0, y)], col)
    a(f'<circle cx="{ux0}" cy="{y}" r="4" fill="{col}"/>')
    text(ux0 + 10, y + 4, lab, 12)
text(820, py(19) - 8, "pasangan twisted, ≤10 cm", 10, "middle", C["sub"], italic=True)
yv = py(18)
a(f'<circle cx="{ux0}" cy="{yv}" r="4" fill="{C["pwr"]}"/>')
text(ux0 + 10, yv + 4, "VBUS", 12)

# power chain, far right
ax0, ax1 = 1060, 1210
box(ax0, 90, ax1 - ax0, 70, "Adaptor 5 V 3 A", "teregulasi, cabang HDD")
xp, xn = 1100, 1180            # + and - leads
text(xp, 176, "+5V", 11, "middle", C["pwr"], "bold")
text(xn, 176, "GND", 11, "middle", C["gnd"], "bold")
line([(xp, 160), (xp, 250)], C["pwr"], 3)
# PTC fuse
a(f'<rect x="{xp-10}" y="250" width="20" height="44" rx="3" fill="#fff" stroke="{C["pwr"]}" stroke-width="2"/>')
line([(xp - 14, 290), (xp + 14, 254)], C["pwr"], 1.5)
text(xp - 18, 268, "F1 PTC", 11, "end", weight="bold")
text(xp - 18, 283, "hold ≈1,1–1,5 A", 10, "end", C["sub"])
line([(xp, 294), (xp, 360)], C["pwr"], 3)
# optional load switch
a(f'<rect x="{xp-34}" y="360" width="68" height="40" rx="4" fill="#fff" stroke="{C["pwr"]}" '
  'stroke-width="1.5" stroke-dasharray="4 3"/>')
text(xp, 378, "S1 saklar", 10, "middle")
text(xp, 392, "(opsional)", 10, "middle", C["sub"])
line([(xp, 400), (xp, 470)], C["pwr"], 3)
dot(xp, 470, C["pwr"])
# bulk capacitor between VBUS node and GND
line([(xp, 470), (xp + 40, 470), (xp + 40, 486)], C["pwr"], 2)
line([(xp + 28, 486), (xp + 52, 486)], C["ink"], 2.5)
a(f'<path d="M {xp+28} 496 Q {xp+40} 490 {xp+52} 496" fill="none" stroke="{C["ink"]}" stroke-width="2.5"/>')
line([(xp + 40, 494), (xp + 40, 520), (xn, 520)], C["gnd"], 2)
text(xp - 10, 500, "C1 150 µF + 100 nF", 10, "end", weight="bold")
text(xp - 10, 513, "dekat konektor USB-A", 10, "end", C["sub"])
line([(xn, 160), (xn, 520)], C["gnd"], 3)
dot(xn, 520, C["gnd"])
# VBUS to USB-A
line([(xp, 470), (xp, yv), (ux1, yv)], C["pwr"], 3)
# Adapter GND to USB-A GND (common ground)
line([(xn, 520), (xn, py(21)), (ux1, py(21))], C["gnd"], 3)
text(xn - 6, py(21) - 8, "ground bersama", 10, "end", C["sub"], italic=True)
# USB-A -> HDD cable
hx = (ux0 + ux1) / 2
line([(hx, py(22) + 24), (hx, py(22) + 70)], "#5f6368", 7)
text(hx + 10, py(22) + 52, "kabel USB-A ↔ Micro-B USB 3.0", 10, "start", C["sub"])
box(ux0 - 20, py(22) + 70, ux1 - ux0 + 40, 70, "ORICO 2020U3 + WD5000LPCX", "500 GB NTFS · 5 V 0,55 A (puncak ≈1 A)")

# PC on COM
line([(BX0 + 70, by + 30), (BX0 + 70, by + 64)], "#5f6368", 6)
box(BX0 - 10, by + 64, 160, 56, "PC Windows", "flash + log (COM)")
text(BX1 - 70, by + 46, "kosongkan pada N0", 10, "middle", C["warn"], italic=True)

# ---------------- legend ----------------
lx, ly = 24, H - 120
a(f'<rect x="{lx}" y="{ly-22}" width="440" height="130" rx="8" fill="#f8f9fa" stroke="#dadce0"/>')
text(lx + 12, ly - 4, "Legenda", 12, weight="bold")
items = (("3V3 / 5V", C["pwr"]), ("GND", C["gnd"]), ("LCD SPI", C["spi"]), ("Backlight", C["bl"]),
         ("Tombol", C["btn"]), ("Encoder", C["enc"]), ("USB D+/D−", C["usb"]))
for k, (lab, col) in enumerate(items):
    x = lx + 12 + (k % 4) * 106
    y = ly + 18 + (k // 4) * 22
    line([(x, y), (x + 26, y)], col, 3)
    text(x + 32, y + 4, lab, 11)
a(f'<circle cx="{lx+18}" cy="{ly+64}" r="6" fill="#fbbc04" stroke="{C["boardline"]}"/>')
text(lx + 30, ly + 68, "pin dipakai", 11)
text(lx + 120, ly + 68, "GPIO0/3/45/46, 35–37", 11, color=C["warn"], weight="bold")
text(lx + 262, ly + 68, "hindari (strapping/PSRAM)", 11, color=C["sub"])
text(lx + 12, ly + 92, "Semua logika 3,3 V. Adaptor 5 V TIDAK ke pin 5V board pada M2/N0.", 11, color=C["sub"])

a("</svg>")
open(sys.argv[1], "w", encoding="utf-8", newline="\n").write("\n".join(out) + "\n")
