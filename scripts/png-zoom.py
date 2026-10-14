#!/usr/bin/env python3
"""Enlarges a PNG by a whole factor, nearest neighbour, so a close-up (`web-drive.mjs` crop) shows single pixels.

Usage: scripts/png-zoom.py <in.png> <out.png> <factor>. 8-bit RGB or RGBA PNGs (what Chromium writes); no image
library needed (the VM has none).
"""
import struct
import sys
import zlib


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def read(path):
    """Width, height, bytes per pixel and the unfiltered rows."""
    data = open(path, "rb").read()
    at, idat = 8, b""
    while at < len(data):
        length, kind = struct.unpack(">I4s", data[at : at + 8])
        chunk = data[at + 8 : at + 8 + length]
        at += 12 + length
        if kind == b"IHDR":
            width, height, depth, colour = struct.unpack(">IIBB", chunk[:10])
            if depth != 8 or colour not in (2, 6):
                sys.exit("only 8-bit RGB or RGBA PNGs")
        elif kind == b"IDAT":
            idat += chunk
    bpp = 4 if colour == 6 else 3
    raw, stride = zlib.decompress(idat), width * bpp
    rows, prev, at = [], bytearray(stride), 0
    for _ in range(height):
        kind, line = raw[at], bytearray(raw[at + 1 : at + 1 + stride])
        at += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            line[x] = (line[x] + (0, a, b, (a + b) // 2, paeth(a, b, c))[kind]) & 255
        rows.append(line)
        prev = line
    return width, height, bpp, rows


def write(path, width, height, bpp, rows, k):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    lines = []
    for row in rows:
        wide = b"".join(bytes(row[x * bpp : (x + 1) * bpp]) * k for x in range(width))
        lines.extend([b"\0" + wide] * k)
    header = struct.pack(">IIBBBBB", width * k, height * k, 8, 6 if bpp == 4 else 2, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(b"".join(lines))) + chunk(b"IEND", b""))


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit("usage: png-zoom.py <in.png> <out.png> <factor>")
    w, h, bpp, rows = read(sys.argv[1])
    write(sys.argv[2], w, h, bpp, rows, int(sys.argv[3]))
