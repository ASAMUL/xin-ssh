"""首页「继续会话」卡上那只趴在服务器上打盹的猫：手写的 Lottie。

不是从 LottieFiles 下载的素材，是这个脚本一笔一笔搭出来的；要改动作、配色，改这里重新生成：

    python scripts/build-cat-napping-lottie.py
        -> src/assets/lottie/cat-napping.lottie（只用标准库）

    python scripts/build-cat-napping-lottie.py --frames <目录> [--dark] [帧号 ...]
        -> 导出几帧 SVG 预览（需要 pip install lottie；转 PNG 可用 @resvg/resvg-js）

一条时间线上排四段，用 marker 切换（LottieAnimation 的 marker / transitions）：
    nap    0–360    打盹，6 秒循环
    wake   360–408  惊醒：睁眼、抬头、竖耳朵、z 散掉、头顶蹦个「!」
    awake  408–528  醒着，2 秒循环：歪头盯着服务器、尾巴甩得急、绿灯忙闪、机箱冒热气
    sleep  528–588  睡回去：先耷拉眼皮，再低头闭眼，叹口气
wake 从 nap 第 0 帧的样子出发、落在 awake 第 0 帧；sleep 反过来。中途切换只会差一点点（被吓醒的那一下）。

服务器和投影挂了颜色槽（server-body / server-line / server-detail / shadow），
深色模式由 HomeSessionCard 传给 LottieAnimation 的 darkSlots 换色，一个文件管两套明暗。
"""
import json
import math
import os
import sys
import zipfile

FPS = 60
NAP, WAKE, AWAKE, SLEEP = 360, 48, 120, 60
WAKE_AT = NAP
AWAKE_AT = WAKE_AT + WAKE
SLEEP_AT = AWAKE_AT + AWAKE
TOTAL = SLEEP_AT + SLEEP
W, H = 280, 200
ANIMATION_ID = "cat-napping"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUTPUT = os.path.join(ROOT, "src", "assets", "lottie", f"{ANIMATION_ID}.lottie")


def hex_rgba(value, alpha=1.0):
    value = value.lstrip("#")
    r, g, b = (int(value[i:i + 2], 16) / 255 for i in (0, 2, 4))
    return [round(r, 4), round(g, 4), round(b, 4), alpha]


PALETTE = {
    "cat": "#F7BE8A",
    "stripe": "#EA9A5C",
    "cream": "#FFF3E3",
    "paw_line": "#EBCFAE",
    "pink": "#F7A9C2",
    "nose": "#EC7EA5",
    "line": "#6A4A3A",
    "pupil": "#3A2A22",
    "white": "#FFFFFF",
    "blush": "#F59BB8",
    "zzz": "#A99CF0",
    "alert": "#E26A98",
    "heat": "#F09A6A",
    "led_green": "#34B26E",
    "led_rose": "#E26A98",
}

SLOTS = {
    "server-body": hex_rgba("#EEEAF0"),
    "server-line": hex_rgba("#D2CBD6"),
    "server-detail": hex_rgba("#DDD6E2"),
    "shadow": hex_rgba("#1C1C20"),
}
DARK_SLOTS = {
    "server-body": hex_rgba("#3A3A41"),
    "server-line": hex_rgba("#56565F"),
    "server-detail": hex_rgba("#4B4B53"),
    "shadow": hex_rgba("#000000"),
}


# ---------------------------------------------------------------- easing

def clamp01(x):
    return max(0.0, min(1.0, x))


def seg(u, start, end):
    """u 在 [start, end] 里走到了几成。"""
    return clamp01((u - start) / (end - start))


def smooth(u):
    return 0.5 - 0.5 * math.cos(math.pi * clamp01(u))


def back_out(u):
    # 冲过头一点再落回来：睁眼、竖耳朵的「弹」
    u = clamp01(u) - 1
    return 1 + 2.70158 * u ** 3 + 1.70158 * u ** 2


def wave(t, period, phase=0.0):
    return math.sin(2 * math.pi * t / period + phase)


def breath(t, period):
    return 0.5 - 0.5 * math.cos(2 * math.pi * t / period)


def lerp(a, b, k):
    if isinstance(a, list):
        return [x + (y - x) * k for x, y in zip(a, b)]
    return a + (b - a) * k


# ---------------------------------------------------------------- tracks

def static(value):
    return {"a": 0, "k": value}


def track(nap, awake=None, wake=None, sleep=None, nap_step=6, hold=False):
    """一个属性在四段里的值。nap / awake 收本段帧号，wake / sleep 收 0~1 的进度；
    没给的段：awake 停在 nap 第 0 帧，过渡段在两头之间缓动。"""
    awake = awake or (lambda t: nap(0))
    wake = wake or (lambda u: lerp(nap(0), awake(0), smooth(u)))
    sleep = sleep or (lambda u: lerp(awake(0), nap(0), smooth(u)))

    samples = {}

    def add(start, length, func, step, progress):
        points = list(range(0, length, step)) + [length]
        for k in points:
            samples[start + k] = func(k / length) if progress else func(k)

    add(0, NAP, nap, 1 if hold else nap_step, False)
    add(WAKE_AT, WAKE, wake, 1 if hold else 2, True)
    add(AWAKE_AT, AWAKE, awake, 1 if hold else 2, False)
    add(SLEEP_AT, SLEEP, sleep, 1 if hold else 2, True)

    def norm(value):
        value = value if isinstance(value, list) else [value]
        return [round(v, 3) for v in value]

    times = sorted(samples)
    values = [norm(samples[t]) for t in times]
    if all(v == values[0] for v in values):
        first = values[0]
        return static(first if len(first) > 1 else first[0])

    keys = []
    for index, (t, value) in enumerate(zip(times, values)):
        if hold:
            if index and value == values[index - 1] and t != TOTAL:
                continue
            keys.append({"t": t, "s": value, "h": 1})
            continue
        # 前后都一样的中间帧是多余的：线性插值本来就停在这个值
        if 0 < index < len(times) - 1 and value == values[index - 1] == values[index + 1]:
            continue
        dims = len(value)
        keys.append({
            "t": t, "s": value,
            "o": {"x": [0.5] * dims, "y": [0.5] * dims},
            "i": {"x": [0.5] * dims, "y": [0.5] * dims},
        })
    keys[-1] = {"t": keys[-1]["t"], "s": keys[-1]["s"]}
    return {"a": 1, "k": keys}


def const(value):
    return lambda _t: value


# ---------------------------------------------------------------- shapes

def transform(p=(0, 0), a=(0, 0), s=(100, 100), r=0, o=100):
    def prop(value):
        return value if isinstance(value, dict) else static(list(value) if isinstance(value, tuple) else value)

    return {
        "ty": "tr",
        "p": prop(p), "a": prop(a), "s": prop(s), "r": prop(r), "o": prop(o),
        "sk": static(0), "sa": static(0),
    }


def leaf(name, items, **tr):
    """Shapes followed by their fill / stroke — already Lottie order (a style paints the shapes above it)."""
    return {"ty": "gr", "nm": name, "it": list(items) + [transform(**tr)]}


def stack(name, children_bottom_to_top, **tr):
    """Only child groups. Lottie draws earlier items on top, so the bottom-to-top list is reversed.
    No styles at this level: a parent's fill would also paint every child group above it."""
    assert all(child["ty"] == "gr" for child in children_bottom_to_top), name
    return {"ty": "gr", "nm": name, "it": list(reversed(children_bottom_to_top)) + [transform(**tr)]}


def color_prop(color, slot):
    if slot:
        prop = static(SLOTS[slot])
        prop["sid"] = slot
        return prop
    return static(hex_rgba(PALETTE.get(color, color)))


def fill(color=None, opacity=100, slot=None):
    return {"ty": "fl", "c": color_prop(color, slot), "o": opacity if isinstance(opacity, dict) else static(opacity), "r": 1}


def stroke(color=None, width=1, opacity=100, slot=None):
    return {"ty": "st", "c": color_prop(color, slot), "o": static(opacity), "w": static(width), "lc": 2, "lj": 2, "ml": 4}


def ellipse(cx, cy, w, h):
    return {"ty": "el", "p": static([cx, cy]), "s": static([w, h])}


def rect(cx, cy, w, h, r):
    return {"ty": "rc", "p": static([cx, cy]), "s": static([w, h]), "r": static(r)}


def path(points, closed=False):
    """points: [(x, y, in_dx, in_dy, out_dx, out_dy)] — tangents relative to the vertex."""
    v, i, o = [], [], []
    for point in points:
        ix, iy, ox, oy = (list(point[2:6]) + [0, 0, 0, 0])[:4]
        v.append([point[0], point[1]])
        i.append([ix, iy])
        o.append([ox, oy])
    return {"ty": "sh", "ks": static({"i": i, "o": o, "v": v, "c": closed})}


def line(x1, y1, x2, y2):
    return path([(x1, y1), (x2, y2)])


# ---------------------------------------------------------------- scene

def shadow():
    return leaf("shadow", [ellipse(140, 186, 206, 10), fill(opacity=10, slot="shadow")])


def led_blink(nap_off, awake_phase):
    """绿灯：打盹时偶尔快闪两下（有数据在走），醒着时忙闪（机器在使劲）。"""
    def nap(t):
        return 25 if any(start <= t < end for start, end in nap_off) else 100

    def awake(t):
        return 25 if (t + awake_phase) % 18 < 6 else 100

    return track(nap, awake, wake=const(100), sleep=const(100), hold=True)


def server_unit(cy, blink):
    return stack(f"unit {cy}", [
        leaf("bay", [rect(140, cy, 72, 7, 3.5), fill(slot="server-detail")]),
        leaf("led green", [ellipse(74, cy, 6.4, 6.4), fill("led_green", opacity=blink)]),
        leaf("led rose", [ellipse(85, cy, 6.4, 6.4), fill("led_rose")]),
        leaf("vents", [
            line(200, cy - 4, 200, cy + 4), line(207, cy - 4, 207, cy + 4), line(214, cy - 4, 214, cy + 4),
            stroke(width=2.2, slot="server-detail"),
        ]),
    ])


def server():
    return stack("server", [
        leaf("case", [rect(140, 150, 190, 60, 10), fill(slot="server-body"), stroke(width=1.6, slot="server-line")]),
        leaf("divider", [line(46, 150, 234, 150), stroke(width=1.4, slot="server-line")]),
        server_unit(135, led_blink([(40, 46), (52, 58), (210, 216)], 0)),
        server_unit(165, led_blink([(130, 136), (268, 274), (280, 286)], 9)),
    ])


def heat(x, offset):
    """醒着时机箱右边冒的热气：一道竖着的波浪线往上飘、淡出，一秒一缕。"""
    def phase(t):
        return ((t + offset) % 60) / 60

    def position(t):
        k = phase(t)
        return [x + 2 * math.sin(2 * math.pi * k), 146 - 30 * k]

    def opacity(t):
        k = phase(t)
        return 85 * min(1, k / 0.25) * min(1, (1 - k) / 0.4)

    squiggle = path([(0, 9, 0, 0, -5, -3), (0, 0, -5, 3, 5, -3), (0, -9, 5, 3, 0, 0)])
    return leaf(f"heat {x}", [squiggle, stroke("heat", 2.2)],
                p=track(const(position(0)), position),
                s=track(const([90, 90]), lambda t: [90 + 25 * phase(t)] * 2),
                o=track(const(0), opacity))


def tail():
    curve = [
        (82, 108, 0, 0, -12, 0),
        (52, 114, 9, -4, -8, 4),
        (42, 134, 0, -9, 0, 8),
        (55, 150, -8, -2, 0, 0),
    ]
    sway = track(
        nap=lambda t: 5 * wave(t, NAP),
        awake=lambda t: 13 * wave(t, 60),
        wake=lambda u: -9 * math.sin(math.pi * u),
        sleep=lambda u: 4 * math.sin(math.pi * u),
    )
    return stack("tail", [
        leaf("tail", [path(curve), stroke("cat", 11)]),
        leaf("tip", [ellipse(55, 150, 11.5, 11.5), fill("cream")]),
    ], p=(82, 108), a=(82, 108), r=sway)


def body():
    stripes = [path([(x - 3, 81, 0, 0, 1, 4), (x + 1, 92, 0, -4, 0, 0)]) for x in (106, 124, 142)]
    scale = track(
        nap=lambda t: [100 + 1.2 * breath(t, 180), 100 + 4.5 * breath(t, 180)],
        # 醒着喘得快也浅
        awake=lambda t: [100 + 0.8 * breath(t, 60), 100 + 2.5 * breath(t, 60)],
        # 吓一跳鼓一下
        wake=lambda u: [100, 100 + 3 * math.sin(math.pi * u)],
        # 睡回去之前长长叹一口气
        sleep=lambda u: [100 + 1.2 * math.sin(math.pi * u), 100 + 5 * math.sin(math.pi * u)],
    )
    return stack("body", [
        leaf("loaf", [rect(134, 102, 124, 44, 22), fill("cat")]),
        leaf("belly", [ellipse(150, 120, 70, 10), fill("cream", 70)]),
        leaf("stripes", stripes + [stroke("stripe", 3.6)]),
    ], p=(134, 124), a=(134, 124), s=scale)


def paws():
    return leaf("paws", [
        rect(189, 119, 17, 10, 5), rect(208, 119, 17, 10, 5), fill("cream"), stroke("paw_line", 1.2),
    ])


def ear_perk():
    return track(
        nap=const([100, 100]),
        awake=const([100, 112]),
        wake=lambda u: [100, 100 + 12 * back_out(seg(u, 0.05, 0.5))],
        sleep=lambda u: [100, 112 - 12 * smooth(seg(u, 0.2, 0.8))],
    )


def ear_flick(t):
    # 打盹时右耳一圈弹一下：睡着的猫被什么声音扰到，耳朵抖一下又放下
    if 200 <= t < 206:
        return -14 * smooth((t - 200) / 6)
    if 206 <= t < 214:
        return lerp(-14, 3, smooth((t - 206) / 8))
    if 214 <= t < 222:
        return lerp(3, 0, smooth((t - 214) / 8))
    return 0


def ear(name, outer, inner, base, rotation=0):
    return stack(name, [
        leaf("outer", [path(outer, closed=True), fill("cat"), stroke("cat", 4)]),
        leaf("inner", [path(inner, closed=True), fill("pink"), stroke("pink", 2)]),
    ], p=base, a=base, s=ear_perk(), r=rotation)


def eye_arc(x, y):
    return path([(x - 5, y, 0, 0, 2.5, 3.6), (x + 5, y, -2.5, 3.6, 0, 0)])


def blink(t, start, length=10):
    # 0 → 1 → 0 的三角，醒着时眨一下眼
    if not start <= t < start + length:
        return 0
    k = (t - start) / length
    return 1 - abs(2 * k - 1)


def eyes():
    closed = leaf("eyes closed", [eye_arc(188, 88), eye_arc(212, 88), stroke("line", 2.2)], o=track(
        nap=const(100),
        awake=const(0),
        wake=lambda u: 100 * (1 - seg(u, 0, 0.15)),
        sleep=lambda u: 100 * seg(u, 0.72, 0.85),
    ))
    opened = stack("eyes open", [
        leaf("pupils", [ellipse(188, 88, 7.5, 9), ellipse(212, 88, 7.5, 9), fill("pupil")]),
        leaf("glints", [ellipse(189.8, 85.6, 2.6, 2.6), ellipse(213.8, 85.6, 2.6, 2.6), fill("white")]),
    ], p=(200, 88), a=(200, 88), s=track(
        nap=const([100, 10]),
        awake=lambda t: [100, 100 - 90 * blink(t, 70)],
        wake=lambda u: [100, 10 + 90 * back_out(seg(u, 0.1, 0.5))],
        # 先耷拉成一条缝撑一会儿，再合上
        sleep=lambda u: [100, 100 - 55 * smooth(seg(u, 0, 0.3)) - 35 * smooth(seg(u, 0.62, 0.82))],
    ), o=track(
        nap=const(0),
        awake=const(100),
        wake=lambda u: 100 * seg(u, 0.05, 0.15),
        sleep=lambda u: 100 * (1 - seg(u, 0.8, 0.86)),
    ))
    return [closed, opened]


def head():
    # 绕脖子转：醒着时抬头、歪头（好奇地盯着服务器），打盹时跟着呼吸轻轻起伏
    neck = (195, 110)
    position = track(
        nap=lambda t: [neck[0], neck[1] - 1.6 * breath(t, 180)],
        awake=lambda t: [neck[0], neck[1] - 8 - 0.8 * breath(t, 60)],
        wake=lambda u: [neck[0], neck[1] - 8 * back_out(seg(u, 0, 0.55))],
        sleep=lambda u: [neck[0], neck[1] - 8 * (1 - smooth(seg(u, 0.1, 0.8)))],
    )
    tilt = track(
        nap=const(0),
        awake=lambda t: -7 + 1.5 * wave(t, AWAKE),
        wake=lambda u: -7 * smooth(seg(u, 0.15, 0.7)),
        sleep=lambda u: -7 * (1 - smooth(seg(u, 0.05, 0.75))),
    )
    right_ear_turn = track(nap=ear_flick, awake=lambda t: 6 * wave(t, AWAKE), nap_step=2)
    return stack("head", [
        ear("ear left", [(180, 70), (183, 46), (197, 61)], [(184, 65), (186, 52), (193, 61)], base=(189, 66)),
        ear("ear right", [(203, 61), (217, 46), (220, 70)], [(207, 61), (214, 52), (216, 65)], base=(212, 66),
            rotation=right_ear_turn),
        leaf("skull", [ellipse(200, 86, 54, 52), fill("cat")]),
        leaf("forehead", [line(194, 63, 195, 70), line(200, 61, 200, 69), line(206, 63, 205, 70), stroke("stripe", 2.6)]),
        leaf("muzzle", [ellipse(200, 98, 32, 18), fill("cream")]),
        leaf("blush", [ellipse(182, 96, 9, 5), ellipse(218, 96, 9, 5), fill("blush", 55)]),
        *eyes(),
        leaf("nose", [path([(196.8, 93.2), (203.2, 93.2), (200, 96.8)], closed=True), fill("nose"), stroke("nose", 1.6)]),
        leaf("mouth", [
            path([(195.8, 98.6, 0, 0, 0.6, 2.4), (200, 98.8, -0.8, 2.4, 0.8, 2.4), (204.2, 98.6, -0.6, 2.4, 0, 0)]),
            stroke("line", 1.4),
        ]),
    ], p=position, a=neck, r=tilt)


def zee(offset, size):
    life = 240  # 每个 z 飘 4 秒

    def phase(t):
        return ((t - offset) % NAP) / life

    def position(t):
        k = min(phase(t), 1)
        return [226 + 26 * k + 3 * math.sin(k * math.pi * 2), 54 - 40 * k]

    def opacity(t):
        k = phase(t)
        if k >= 1:
            return 0
        return 100 * min(1, k / 0.2) * min(1, (1 - k) / 0.35)

    def scale(t):
        k = min(phase(t), 1)
        return [60 + 50 * k, 60 + 50 * k]

    half = size / 2
    glyph = path([(-half, -half), (half, -half), (-half, half), (half, half)])
    start = opacity(0)
    return leaf(f"z {offset}", [glyph, stroke("zzz", 2.4)],
                p=track(position), s=track(scale),
                o=track(
                    nap=opacity,
                    awake=const(0),
                    # 一惊醒 z 就散了；睡回去最后几帧再从原处浮现，接上打盹的第 0 帧
                    wake=lambda u: start * (1 - seg(u, 0, 0.2)),
                    sleep=lambda u: start * seg(u, 0.6, 1),
                ))


def exclaim():
    """惊醒那一下头顶蹦出的「!」，只在 wake 段里出现。"""
    scale = track(nap=const([0, 0]), awake=const([0, 0]),
                  wake=lambda u: [100 * back_out(seg(u, 0.08, 0.4))] * 2 if u < 0.97 else [0, 0],
                  sleep=const([0, 0]))
    opacity = track(nap=const(0), awake=const(0),
                    wake=lambda u: 100 * (1 - seg(u, 0.65, 0.95)) if u > 0.04 else 0,
                    sleep=const(0))
    return stack("exclaim", [
        leaf("bar", [line(0, -9, 0, 2), stroke("alert", 4.2)]),
        leaf("dot", [ellipse(0, 8.5, 5, 5), fill("alert")]),
    ], p=(236, 50), a=(0, 8), r=14, s=scale, o=opacity)


def build():
    scene = stack("scene", [
        shadow(), server(), heat(248, 0), heat(261, 30), tail(), body(), paws(), head(),
        zee(0, 9), zee(120, 9), zee(240, 9), exclaim(),
    ])
    layer = {
        "ddd": 0, "ind": 1, "ty": 4, "nm": "cat napping", "sr": 1,
        "ks": {
            "o": static(100), "r": static(0), "p": static([0, 0, 0]),
            "a": static([0, 0, 0]), "s": static([100, 100, 100]),
        },
        "ao": 0, "shapes": [scene],
        "ip": 0, "op": TOTAL, "st": 0, "bm": 0,
    }
    return {
        "v": "5.7.4", "fr": FPS, "ip": 0, "op": TOTAL, "w": W, "h": H,
        "nm": "cat napping", "ddd": 0, "assets": [], "layers": [layer],
        "markers": [
            {"tm": 0, "cm": "nap", "dr": NAP},
            {"tm": WAKE_AT, "cm": "wake", "dr": WAKE},
            {"tm": AWAKE_AT, "cm": "awake", "dr": AWAKE},
            {"tm": SLEEP_AT, "cm": "sleep", "dr": SLEEP},
        ],
        "slots": {sid: {"p": {"a": 0, "k": color}} for sid, color in SLOTS.items()},
    }


def write_dotlottie(animation, path):
    # 和另外两只猫同一种包：manifest + animations/<id>.json
    manifest = {
        "version": "1",
        "generator": "scripts/build-cat-napping-lottie.py",
        "animations": [{"id": ANIMATION_ID, "mode": "normal", "direction": 1}],
    }
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("manifest.json", json.dumps(manifest, separators=(",", ":")))
        archive.writestr(f"animations/{ANIMATION_ID}.json", json.dumps(animation, separators=(",", ":")))


def export_frames(frames_dir, frames, dark):
    import lottie
    from lottie.exporters.svg import export_svg

    if dark:
        SLOTS.update(DARK_SLOTS)
    preview = build()
    # python-lottie 不认 slots / markers，预览直接用属性上写死的默认色（深色时上面已经换掉）
    preview.pop("slots")
    preview.pop("markers")
    model = lottie.objects.Animation.load(preview)
    os.makedirs(frames_dir, exist_ok=True)
    for name in os.listdir(frames_dir):
        os.remove(os.path.join(frames_dir, name))
    for frame in frames:
        export_svg(model, os.path.join(frames_dir, f"f{frame:03d}.svg"), frame)
    print("frames", frames, "->", frames_dir)


def main():
    args = sys.argv[1:]
    if "--frames" in args:
        index = args.index("--frames")
        frames_dir = args[index + 1]
        rest = args[:index] + args[index + 2:]
        dark = "--dark" in rest
        frames = [int(x) for x in rest if x != "--dark"] or [0, 200, 366, 380, 420, 478, 540, 575]
        export_frames(frames_dir, frames, dark)
        return

    write_dotlottie(build(), OUTPUT)
    print("wrote", os.path.relpath(OUTPUT, ROOT), os.path.getsize(OUTPUT), "bytes")


if __name__ == "__main__":
    main()
