"""Terra BLE test harness for M3/M4 acceptance (development tool, not the companion).

Setup (once):
    python -m venv .tools/ble-test-venv
    .tools/ble-test-venv/Scripts/python -m pip install -r scripts/ble-test-requirements.txt

Usage:
    .tools/ble-test-venv/Scripts/python scripts/ble-test.py scan
    .tools/ble-test-venv/Scripts/python scripts/ble-test.py test [--hold 20]
    .tools/ble-test-venv/Scripts/python scripts/ble-test.py send --title "..." --artist "..." [--paused]

`test` runs the M3/M4 acceptance sequence: discover, connect, discover the
service, subscribe TX, receive HELLO, answer HELLO_ACK, write RX (volume and a
fragmented MEDIA_STATE), then checks that malformed input is rejected with
ERROR while the device keeps running. Exit code 0 only if every check passed.
"""
import argparse
import asyncio
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import terra_protocol as p  # noqa: E402
from bleak import BleakClient, BleakScanner  # noqa: E402

SERVICE = "6f3a0001-9b21-4c71-9a8b-31a8b9f10001"
RX = "6f3a0002-9b21-4c71-9a8b-31a8b9f10001"
TX = "6f3a0003-9b21-4c71-9a8b-31a8b9f10001"
STATUS = "6f3a0004-9b21-4c71-9a8b-31a8b9f10001"
LINKS = ["ADVERTISING", "CONNECTED", "READY", "PROTOCOL_MISMATCH"]
COMPANION_VERSION = (0, 1, 0)

failures = 0


def check(condition: bool, label: str, detail: str = "") -> bool:
    global failures
    print(f"{'PASS' if condition else 'FAIL'}  {label}{' - ' + detail if detail else ''}")
    failures += 0 if condition else 1
    return condition


async def find(timeout: float):
    device = await BleakScanner.find_device_by_filter(
        lambda d, adv: (d.name == "Terra") or SERVICE in [u.lower() for u in adv.service_uuids],
        timeout=timeout)
    return device


class Link:
    def __init__(self, client: BleakClient, packet_mtu: int = 0):
        self.client = client
        self.packet_mtu = packet_mtu  # 0 = negotiated MTU; smaller forces fragmentation
        self.reassembler = p.Reassembler()
        self.messages: asyncio.Queue = asyncio.Queue()
        self.seq = 0
        self.start = time.monotonic()

    def on_notify(self, _sender, data: bytearray) -> None:
        now = int((time.monotonic() - self.start) * 1000)
        result = self.reassembler.push(bytes(data), now)
        if result.drops:
            print(f"      notification dropped: {result.drops}")
        if result.message:
            self.messages.put_nowait(result.message)

    @property
    def mtu(self) -> int:
        negotiated = max(23, min(self.client.mtu_size, 247))
        return min(negotiated, self.packet_mtu) if self.packet_mtu else negotiated

    async def send(self, msg_type: int, payload: bytes, *, version: int = p.VERSION) -> None:
        packets = p.fragment(msg_type, self.seq, payload, self.mtu)
        self.seq = (self.seq + 1) & 0xFF
        for packet in packets:
            if version != p.VERSION:
                packet = bytes([version]) + packet[1:]
            await self.client.write_gatt_char(RX, packet, response=True)

    async def receive(self, timeout: float):
        try:
            return await asyncio.wait_for(self.messages.get(), timeout)
        except asyncio.TimeoutError:
            return None

    async def status(self):
        return p.decode_status(bytes(await self.client.read_gatt_char(STATUS)))


async def connect_and_handshake(address, args) -> tuple[BleakClient, Link] | None:
    client = BleakClient(address)
    await client.connect()
    link = Link(client, args.mtu)
    check(client.is_connected, "connect", f"MTU {client.mtu_size}")
    service = client.services.get_service(SERVICE)
    if not check(service is not None, "discover service", SERVICE):
        await client.disconnect()
        return None
    props = {c.uuid.lower(): set(c.properties) for c in service.characteristics}
    check({"write", "write-without-response"} <= props.get(RX, set()), "RX characteristic", str(props.get(RX)))
    check("notify" in props.get(TX, set()), "TX characteristic", str(props.get(TX)))
    check({"read", "notify"} <= props.get(STATUS, set()), "STATUS characteristic", str(props.get(STATUS)))
    status = await link.status()
    check(LINKS[status[4]] == "CONNECTED", "status before handshake", LINKS[status[4]])

    await client.start_notify(TX, link.on_notify)
    message = await link.receive(3.0)
    if not check(message is not None and message[0] == p.HELLO, "subscribe TX -> HELLO received"):
        await client.disconnect()
        return None
    version, major, minor, patch, caps = p.decode_hello(message[2])
    check(version == p.VERSION, "HELLO protocol version", f"protocol={version} firmware={major}.{minor}.{patch} caps=0x{caps:04x}")
    await link.send(p.HELLO_ACK, p.encode_hello_ack(*COMPANION_VERSION))
    await asyncio.sleep(0.3)
    status = await link.status()
    check(LINKS[status[4]] == "READY", "HELLO_ACK -> READY", f"rx_errors={status[6]}")
    return client, link


def media_from(args) -> p.MediaState:
    playback = p.PAUSED if args.paused else p.PLAYING
    return p.MediaState(playback, args.position, args.duration, args.source, args.title, args.artist,
                        args.album)


async def cmd_scan(args) -> int:
    devices = await BleakScanner.discover(timeout=args.timeout, return_adv=True)
    found = 0
    for device, adv in devices.values():
        if device.name == "Terra" or SERVICE in [u.lower() for u in adv.service_uuids]:
            print(f"{device.address}  RSSI {adv.rssi}  name={device.name}  services={adv.service_uuids}")
            found += 1
    print(f"{found} Terra device(s) found")
    return 0 if found else 1


async def cmd_test(args) -> int:
    device = await find(args.timeout)
    if not check(device is not None, "discover device", "advertising name Terra"):
        return 1
    print(f"      address {device.address}")
    session = await connect_and_handshake(device, args)
    if session is None:
        return 1
    client, link = session
    try:
        await link.send(p.VOLUME_STATE, p.encode_volume(72, False))
        media = p.MediaState(p.PLAYING, 81200, 216000, "Spotify", "The Less I Know The Better",
                             "Tame Impala", "Currents")
        payload = media.encode()
        packets = len(p.fragment(p.MEDIA_STATE, 0, payload, link.mtu))
        await link.send(p.MEDIA_STATE, payload)
        reply = await link.receive(1.0)
        check(reply is None, "write RX: VOLUME_STATE + MEDIA_STATE accepted",
              f"{len(payload)} bytes in {packets} packet(s), no ERROR")
        before = (await link.status())[6]

        bad = bytes([p.PLAYING]) + (1).to_bytes(4, "little") + (2).to_bytes(4, "little") + bytes([0, 128, 0, 0]) + b"a" * 128
        await link.send(p.MEDIA_STATE, bad)
        reply = await link.receive(2.0)
        ok = reply is not None and reply[0] == p.ERROR and p.decode_error(reply[2])[0] == 2
        check(ok, "malformed MEDIA_STATE -> ERROR MALFORMED", str(reply and p.decode_error(reply[2])))

        await link.send(p.VOLUME_STATE, p.encode_volume(10, False), version=2)
        reply = await link.receive(2.0)
        ok = reply is not None and reply[0] == p.ERROR and p.decode_error(reply[2])[0] == 1
        check(ok, "foreign protocol version -> ERROR UNSUPPORTED_VERSION", str(reply and p.decode_error(reply[2])))

        await link.client.write_gatt_char(RX, bytes([p.VERSION, p.MEDIA_STATE, 0, 99, 1, 2]), response=True)
        await link.send(p.MEDIA_COMMAND, p.encode_command(p.PLAY_PAUSE))
        reply = await link.receive(2.0)
        ok = reply is not None and reply[0] == p.ERROR and p.decode_error(reply[2])[0] == 3
        check(ok, "orphan fragment ignored, MEDIA_COMMAND from PC -> UNSUPPORTED_TYPE",
              str(reply and p.decode_error(reply[2])))

        status = await link.status()
        check(LINKS[status[4]] == "READY" and status[6] >= before + 4, "device still READY after bad input",
              f"rx_errors {before} -> {status[6]}")

        largest = p.MediaState(p.PLAYING, 1000, 2000, "S" * 31, "é" * 63 + "a", "😀" * 23 + "abc", "ü" * 47 + "x")
        payload = largest.encode()
        packets = len(p.fragment(p.MEDIA_STATE, 0, payload, link.mtu))
        await link.send(p.MEDIA_STATE, payload)
        reply = await link.receive(1.0)
        status = await link.status()
        check(reply is None and status[6] == before + 4, "max-size MEDIA_STATE reassembled",
              f"{len(payload)} bytes in {packets} packet(s)")

        await link.send(p.MEDIA_STATE, media.encode())
        print(f"      LCD should now show: SPOTIFY / {media.title} / {media.artist} / VOL 72, timeline running")
        if args.hold:
            print(f"      holding connection for {args.hold} s for visual check")
            await asyncio.sleep(args.hold / 2)
            paused = p.MediaState(p.PAUSED, 120000, 216000, media.source, media.title, media.artist, media.album)
            await link.send(p.MEDIA_STATE, paused.encode())
            await link.send(p.VOLUME_STATE, p.encode_volume(40, True))
            print("      sent PAUSED at 02:00 and MUTED: LCD should show 'Paused', '02:00 / 03:36', 'MUTED'")
            await asyncio.sleep(args.hold / 2)
    finally:
        await client.disconnect()
    print("      disconnected: LCD should return to 'Waiting for computer...'")
    print(f"{'ALL CHECKS PASSED' if failures == 0 else f'{failures} CHECK(S) FAILED'}")
    return 0 if failures == 0 else 1


async def cmd_send(args) -> int:
    device = await find(args.timeout)
    if device is None:
        print("Terra not found")
        return 1
    session = await connect_and_handshake(device, args)
    if session is None:
        return 1
    client, link = session
    try:
        await link.send(p.VOLUME_STATE, p.encode_volume(args.volume, args.muted))
        await link.send(p.MEDIA_STATE, media_from(args).encode())
        print(f"Sent; holding connection {args.hold} s (Ctrl+C to stop)")
        await asyncio.sleep(args.hold)
    finally:
        await client.disconnect()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--timeout", type=float, default=10.0, help="scan timeout in seconds")
    parser.add_argument("--mtu", type=int, default=0, help="fragment as if the ATT MTU were this (e.g. 23)")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("scan")
    test = sub.add_parser("test")
    test.add_argument("--hold", type=float, default=0, help="seconds to keep the link for visual checks")
    send = sub.add_parser("send")
    send.add_argument("--source", default="Spotify")
    send.add_argument("--title", default="Test Song")
    send.add_argument("--artist", default="Test Artist")
    send.add_argument("--album", default="")
    send.add_argument("--position", type=int, default=60000)
    send.add_argument("--duration", type=int, default=180000)
    send.add_argument("--paused", action="store_true")
    send.add_argument("--volume", type=int, default=72)
    send.add_argument("--muted", action="store_true")
    send.add_argument("--hold", type=float, default=30)
    args = parser.parse_args()
    handler = {"scan": cmd_scan, "test": cmd_test, "send": cmd_send}[args.command]
    return asyncio.run(handler(args))


if __name__ == "__main__":
    sys.exit(main())
