"""The dictz icon pixels (no Qt) — same as the Rust edition's ``icon::app_icon_rgba``."""
from __future__ import annotations


def app_icon_rgba(n: int = 64) -> bytes:
    """The dictz icon (an open book on a blue rounded square) as RGBA pixels —
    the same pixels as the Rust edition's ``icon::app_icon_rgba``."""
    out = bytearray(n * n * 4)
    r = 12.0

    def inside(x: float, y: float) -> bool:
        cx = min(max(x, r), n - r)
        cy = min(max(y, r), n - r)
        return (x - cx) ** 2 + (y - cy) ** 2 <= r * r

    for y in range(n):
        for x in range(n):
            if not inside(x + 0.5, y + 0.5):
                continue
            c = (0x25, 0x63, 0xEB, 0xFF)
            page = 14 <= x <= 30 or 33 <= x <= 49
            top = 18 + ((30 - min(x, 30)) // 6 if x < 32 else (max(x, 33) - 33) // 6)
            if page and top <= y <= 46:
                c = (0xFF, 0xFF, 0xFF, 0xFF)
                line_row = y > top + 4 and (y - top) % 6 == 0 and y < 44
                line_col = 17 <= x <= 27 or 36 <= x <= 46
                if line_row and line_col:
                    c = (0x93, 0xB4, 0xF5, 0xFF)
            i = (y * n + x) * 4
            out[i:i + 4] = bytes(c)
    return bytes(out)
