#!/usr/bin/env python3
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


@dataclass
class AttWrite:
    index: int
    opcode: int
    handle: int
    value: bytes


def read_u32_be(buf: bytes, off: int) -> int:
    return int.from_bytes(buf[off : off + 4], "big")


def read_u64_be(buf: bytes, off: int) -> int:
    return int.from_bytes(buf[off : off + 8], "big")


def iter_btsnoop_records(raw: bytes) -> Iterable[bytes]:
    if len(raw) < 16 or not raw.startswith(b"btsnoop\x00"):
        raise ValueError("Arquivo invalido: cabecalho btsnoop ausente")

    off = 16
    rec = 0
    while off + 24 <= len(raw):
        orig_len = read_u32_be(raw, off)
        incl_len = read_u32_be(raw, off + 4)
        _flags = read_u32_be(raw, off + 8)
        _drops = read_u32_be(raw, off + 12)
        _ts = read_u64_be(raw, off + 16)
        off += 24

        if off + incl_len > len(raw):
            break

        packet = raw[off : off + incl_len]
        off += incl_len
        rec += 1

        if incl_len != orig_len:
            # Mantemos o pacote incluido para analise mesmo se truncado.
            pass

        yield packet


def parse_att_write_from_hci(packet: bytes, idx: int) -> AttWrite | None:
    # Formato esperado: HCI H4 + ACL + L2CAP + ATT
    if len(packet) < 1 or packet[0] != 0x02:
        return None

    # HCI ACL header: handle/pb/bc (2 LE), data_total_len (2 LE)
    if len(packet) < 5:
        return None
    acl_data = packet[5:]
    if len(acl_data) < 4:
        return None

    # L2CAP basic header
    l2cap_len = int.from_bytes(acl_data[0:2], "little")
    cid = int.from_bytes(acl_data[2:4], "little")
    if cid != 0x0004:
        return None

    pdu = acl_data[4 : 4 + l2cap_len]
    if len(pdu) < 3:
        return None

    opcode = pdu[0]
    if opcode not in (0x12, 0x52):
        return None

    handle = int.from_bytes(pdu[1:3], "little")
    value = pdu[3:]
    return AttWrite(index=idx, opcode=opcode, handle=handle, value=value)


def is_mostly_ascii(data: bytes) -> bool:
    if not data:
        return False
    printable = 0
    for b in data:
        if 32 <= b <= 126 or b in (9, 10, 13):
            printable += 1
    return printable / len(data) >= 0.8


def opcode_name(op: int) -> str:
    return "Wr-Req" if op == 0x12 else "Wr-Cmd"


def sample_value_str(data: bytes) -> str:
    hexs = data.hex().upper()
    if is_mostly_ascii(data):
        txt = data.decode("latin-1", errors="replace")
        return f"ASCII:'{txt[:24]}' HEX:{hexs[:48]}"
    return hexs[:48]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Extrai ATT Write Request/Command de arquivo btsnoop_hci.log"
    )
    parser.add_argument("logfile", type=Path, help="Caminho para btsnoop_hci.log")
    parser.add_argument(
        "--top",
        type=int,
        default=10,
        help="Quantidade de handles mais frequentes para mostrar",
    )
    parser.add_argument(
        "--show",
        type=int,
        default=5,
        help="Quantidade de amostras por handle",
    )
    args = parser.parse_args()

    raw = args.logfile.read_bytes()
    writes: list[AttWrite] = []

    for i, pkt in enumerate(iter_btsnoop_records(raw), start=1):
        w = parse_att_write_from_hci(pkt, i)
        if w is not None:
            writes.append(w)

    if not writes:
        print("Nenhum ATT Write (0x12/0x52) encontrado.")
        return 1

    by_handle: dict[int, list[AttWrite]] = defaultdict(list)
    by_opcode = Counter()
    for w in writes:
        by_handle[w.handle].append(w)
        by_opcode[w.opcode] += 1

    print("ATT WRITES VALIDOS")
    print("=" * 80)
    print(f"Total de writes: {len(writes)}")
    print(
        f"Write Req (0x12): {by_opcode.get(0x12, 0)} | "
        f"Write Cmd (0x52): {by_opcode.get(0x52, 0)}"
    )
    print(f"Handles unicos: {len(by_handle)}")
    print()

    top_handles = sorted(by_handle.items(), key=lambda kv: len(kv[1]), reverse=True)[: args.top]
    for handle, items in top_handles:
        print(f"Handle 0x{handle:04X}: {len(items)} comandos")
        for w in items[: args.show]:
            print(
                f"  [{w.index:5d}] {opcode_name(w.opcode)} | len={len(w.value):3d} | "
                f"{sample_value_str(w.value)}"
            )
        if len(items) > args.show:
            print(f"  ... e {len(items) - args.show} mais")
        print()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
