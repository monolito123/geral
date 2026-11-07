#!/usr/bin/env python3
"""Gera os icones do app (rosto do Lula em estilo blocado, igual ao modelo 3D do jogo).

Uso: python3 tools/make_icon.py   (precisa do Pillow)
"""
import os
from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RES = os.path.join(ROOT, "res")

SKIN = (217, 160, 102)
SKIN_D = (201, 141, 85)
HAIR = (207, 207, 207)
BEARD = (236, 236, 236)
MUST = (196, 196, 196)
NAVY = (30, 42, 68)
RED = (208, 28, 31)
GREEN = (22, 163, 74)
YELLOW = (255, 214, 10)
OUT = (25, 25, 25)


def face(d, S, cx, cy, k):
    """Desenha o rosto centrado em (cx, cy); k = escala (1 = cabeca de 0.30*S)."""
    def R(x0, y0, x1, y1, col, outline=False):
        box = [cx + x0 * S * k, cy + y0 * S * k, cx + x1 * S * k, cy + y1 * S * k]
        if outline:
            w = max(2, int(S * k * 0.012))
            d.rectangle([box[0] - w, box[1] - w, box[2] + w, box[3] + w], fill=OUT)
        d.rectangle(box, fill=col)

    # terno + camisa + gravata
    R(-0.30, 0.20, 0.30, 0.42, NAVY, True)
    d.polygon([(cx - 0.07 * S * k, cy + 0.20 * S * k), (cx + 0.07 * S * k, cy + 0.20 * S * k),
               (cx, cy + 0.36 * S * k)], fill=(244, 244, 244))
    R(-0.022, 0.21, 0.022, 0.42, RED)
    # cabeca
    R(-0.15, -0.20, 0.15, 0.17, SKIN, True)
    # orelhas
    R(-0.175, -0.04, -0.15, 0.05, SKIN_D)
    R(0.15, -0.04, 0.175, 0.05, SKIN_D)
    # cabelo
    R(-0.16, -0.235, 0.16, -0.16, HAIR, True)
    R(-0.16, -0.20, -0.135, -0.03, HAIR)
    R(0.135, -0.20, 0.16, -0.03, HAIR)
    # barba
    R(-0.16, 0.04, 0.16, 0.20, BEARD, True)
    R(-0.16, -0.06, -0.12, 0.10, BEARD)
    R(0.12, -0.06, 0.16, 0.10, BEARD)
    # bigode e boca
    R(-0.085, 0.02, 0.085, 0.06, MUST)
    R(-0.04, 0.085, 0.04, 0.105, (122, 46, 46))
    # nariz
    R(-0.03, -0.055, 0.03, 0.03, SKIN_D)
    # olhos
    for s in (-1, 1):
        x0, x1 = sorted((s * 0.035, s * 0.10))
        R(x0, -0.095, x1, -0.06, (255, 255, 255))
        p0, p1 = sorted((s * 0.05, s * 0.085))
        R(p0, -0.092, p1, -0.063, (43, 27, 16))
        e0, e1 = sorted((s * 0.03, s * 0.11))
        R(e0, -0.13, e1, -0.108, (181, 181, 181))


def render(S, adaptive):
    SS = 4
    big = S * SS
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    if adaptive:
        # fundo vem da cor; so circulo amarelo + rosto na zona segura
        r = 0.30 * big
        d.ellipse([big / 2 - r, big / 2 - r, big / 2 + r, big / 2 + r], fill=YELLOW)
        face(d, big, big / 2, big * 0.52, 0.78)
    else:
        m = int(big * 0.04)
        d.rounded_rectangle([m, m, big - m, big - m], radius=int(big * 0.22), fill=GREEN)
        r = 0.38 * big
        d.ellipse([big / 2 - r, big / 2 - r, big / 2 + r, big / 2 + r], fill=YELLOW)
        face(d, big, big / 2, big * 0.52, 1.0)
        # recorta o terno no quadrado arredondado
        mask = Image.new("L", (big, big), 0)
        ImageDraw.Draw(mask).rounded_rectangle([m, m, big - m, big - m], radius=int(big * 0.22), fill=255)
        img.putalpha(Image.composite(img.getchannel("A"), Image.new("L", (big, big), 0), mask))
    return img.resize((S, S), Image.LANCZOS)


def save(img, folder, name):
    p = os.path.join(RES, folder)
    os.makedirs(p, exist_ok=True)
    img.save(os.path.join(p, name), optimize=True)


if __name__ == "__main__":
    save(render(144, False), "mipmap-xxhdpi", "ic_launcher.png")
    save(render(192, False), "mipmap-xxxhdpi", "ic_launcher.png")
    save(render(432, True), "mipmap-xxxhdpi", "ic_launcher_fg.png")
    render(512, False).save(os.path.join(ROOT, "icon-512.png"), optimize=True)
    print("icones gerados")
