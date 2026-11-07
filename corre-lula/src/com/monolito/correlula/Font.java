package com.monolito.correlula;

/**
 * Atlas de fonte (texto branco com contorno preto) + icones desenhados por codigo.
 * O desenho das letras depende da plataforma (Painter); os icones sao rasterizados aqui.
 */
final class Font {
    interface Painter {
        void setup(float px, float stroke);
        float ascent();
        float descent();
        float advance(char c);
        void begin(int w, int h);
        void draw(char c, float x, float baseline);
        int[] finish();
    }

    // Icones (codigos de caractere reservados)
    static final char PICANHA = 1, MAGNET = 2, PLANE = 3, SHOE = 4, PAUSE = 5, SOUND = 6, MUTE = 7,
            STAR = 8, DUCK = 9, CIRCLE = 10, LOCK = 11, CHECK = 12, UP = 14, DOWN = 15, LEFT = 16,
            RIGHT = 17, ROUND = 18, SOFT = 19, WHITE = 20, MULT = 21, PLAY = 22, CART = 23;
    private static final char[] ICONS = {PICANHA, MAGNET, PLANE, SHOE, PAUSE, SOUND, MUTE, STAR, DUCK,
            CIRCLE, LOCK, CHECK, UP, DOWN, LEFT, RIGHT, ROUND, SOFT, WHITE, MULT, PLAY, CART};

    static final String CHARS;
    static {
        StringBuilder sb = new StringBuilder();
        for (char c = 32; c < 127; c++) sb.append(c);
        sb.append("ÀÁÂÃÇÉÊÍÓÔÕÚÜ"
                + "àáâãçéêíóôõúü°ºª");
        CHARS = sb.toString();
    }

    static final float PX = 44f, STROKE = 7f;

    int aw, ah;
    float cellH;
    final float[] u0 = new float[256], v0 = new float[256], u1 = new float[256], v1 = new float[256];
    final float[] adv = new float[256], cw = new float[256], offX = new float[256];
    int[] pixels;

    boolean has(char c) {
        return c < 256 && cw[c] > 0;
    }

    void build(Painter p) {
        p.setup(PX, STROKE);
        float asc = p.ascent(), desc = p.descent();
        cellH = (float) Math.ceil(asc + desc + STROKE + 4);
        int ch = (int) cellH;
        aw = 1024;
        // passe de medida
        int x = 2, y = 2;
        int n = CHARS.length();
        int[] gx = new int[n], gy = new int[n], gw = new int[n];
        for (int i = 0; i < n; i++) {
            char c = CHARS.charAt(i);
            float a = p.advance(c);
            int w = (int) Math.ceil(a + STROKE + 4);
            if (x + w > aw - 2) { x = 2; y += ch + 3; }
            gx[i] = x; gy[i] = y; gw[i] = w;
            adv[c] = a; cw[c] = w; offX[c] = STROKE * 0.5f + 2f;
            x += w + 3;
        }
        int iconY = y + ch + 3, ix = 2;
        int[] icx = new int[ICONS.length], icy = new int[ICONS.length];
        for (int i = 0; i < ICONS.length; i++) {
            if (ix + ch > aw - 2) { ix = 2; iconY += ch + 3; }
            icx[i] = ix; icy[i] = iconY;
            ix += ch + 3;
        }
        int need = iconY + ch + 3;
        ah = need <= 512 ? 512 : 1024;

        p.begin(aw, ah);
        for (int i = 0; i < n; i++) {
            char c = CHARS.charAt(i);
            p.draw(c, gx[i] + STROKE * 0.5f + 2f, gy[i] + 2 + STROKE * 0.5f + asc);
            setUV(c, gx[i], gy[i], gw[i], ch);
        }
        pixels = p.finish();
        for (int i = 0; i < ICONS.length; i++) {
            char c = ICONS[i];
            drawIcon(c, icx[i], icy[i], ch);
            setUV(c, icx[i], icy[i], ch, ch);
            adv[c] = ch * 0.9f;
            cw[c] = ch;
            offX[c] = ch * 0.05f;
        }
        // regiao branca solida: usa o miolo do icone WHITE
        float cxw = (u0[WHITE] + u1[WHITE]) * 0.5f, cyw = (v0[WHITE] + v1[WHITE]) * 0.5f;
        float e = 2f / aw;
        u0[WHITE] = cxw - e; u1[WHITE] = cxw + e;
        v0[WHITE] = cyw - e; v1[WHITE] = cyw + e;
    }

    private void setUV(char c, int x, int y, int w, int h) {
        u0[c] = (x + 0.5f) / aw;
        v0[c] = (y + 0.5f) / ah;
        u1[c] = (x + w - 0.5f) / aw;
        v1[c] = (y + h - 0.5f) / ah;
    }

    /** Largura de texto em pixels do atlas (multiplique pela escala). */
    float width(CharSequence s) {
        return width(s, 0, s.length());
    }

    float width(CharSequence s, int from, int to) {
        float w = 0;
        for (int i = from; i < to; i++) {
            char c = s.charAt(i);
            if (c == '\n') break;
            if (c < 256 && cw[c] > 0) w += adv[c] + STROKE * 0.55f;
        }
        return w;
    }

    // ---------------------------------------------------------------- icones
    private int ox, oy, sz;

    /** Forma definida por teste de ponto em coordenadas normalizadas 0..1 (v para baixo). */
    private abstract static class Sh {
        abstract boolean in(float u, float v);
    }

    private static final class Ell extends Sh {
        final float cx, cy, rx, ry;
        Ell(float cx, float cy, float rx, float ry) { this.cx = cx; this.cy = cy; this.rx = rx; this.ry = ry; }
        boolean in(float u, float v) {
            float a = (u - cx) / rx, b = (v - cy) / ry;
            return a * a + b * b <= 1f;
        }
    }

    private static final class Box extends Sh {
        final float x0, y0, x1, y1;
        Box(float x0, float y0, float x1, float y1) { this.x0 = x0; this.y0 = y0; this.x1 = x1; this.y1 = y1; }
        boolean in(float u, float v) { return u >= x0 && u <= x1 && v >= y0 && v <= y1; }
    }

    private static final class Poly extends Sh {
        final float[] p;
        Poly(float... p) { this.p = p; }
        boolean in(float u, float v) {
            boolean r = false;
            int n = p.length / 2;
            for (int i = 0, j = n - 1; i < n; j = i++) {
                float xi = p[i * 2], yi = p[i * 2 + 1], xj = p[j * 2], yj = p[j * 2 + 1];
                if (((yi > v) != (yj > v)) && (u < (xj - xi) * (v - yi) / (yj - yi) + xi)) r = !r;
            }
            return r;
        }
    }

    private static final class Union extends Sh {
        final Sh[] s;
        Union(Sh... s) { this.s = s; }
        boolean in(float u, float v) {
            for (Sh x : s) if (x.in(u, v)) return true;
            return false;
        }
    }

    private static final class Minus extends Sh {
        final Sh a, b;
        Minus(Sh a, Sh b) { this.a = a; this.b = b; }
        boolean in(float u, float v) { return a.in(u, v) && !b.in(u, v); }
    }

    private static final class And extends Sh {
        final Sh a, b;
        And(Sh a, Sh b) { this.a = a; this.b = b; }
        boolean in(float u, float v) { return a.in(u, v) && b.in(u, v); }
    }

    /** Segmento grosso. */
    private static final class Seg extends Sh {
        final float ax, ay, bx, by, r;
        Seg(float ax, float ay, float bx, float by, float r) { this.ax = ax; this.ay = ay; this.bx = bx; this.by = by; this.r = r; }
        boolean in(float u, float v) {
            float dx = bx - ax, dy = by - ay;
            float t = ((u - ax) * dx + (v - ay) * dy) / (dx * dx + dy * dy);
            t = t < 0 ? 0 : (t > 1 ? 1 : t);
            float px = ax + dx * t - u, py = ay + dy * t - v;
            return px * px + py * py <= r * r;
        }
    }

    private static final int SS = 2;

    private static boolean dil(Sh s, float u, float v, float d) {
        if (s.in(u, v)) return true;
        float e = d * 0.7f;
        return s.in(u + d, v) || s.in(u - d, v) || s.in(u, v + d) || s.in(u, v - d)
                || s.in(u + e, v + e) || s.in(u - e, v + e) || s.in(u + e, v - e) || s.in(u - e, v - e);
    }

    private void fill(Sh s, int argb, float outline) {
        float d = outline;
        float px1 = 1f / sz;
        for (int py = 0; py < sz; py++) {
            for (int px = 0; px < sz; px++) {
                float cu = (px + 0.5f) * px1, cv = (py + 0.5f) * px1;
                // teste rapido no centro do pixel (com folga de ~1 pixel)
                if (d > 0 && !dil(s, cu, cv, d + px1 * 0.75f)) continue;
                int hit = 0;
                for (int sy = 0; sy < SS; sy++) {
                    for (int sx = 0; sx < SS; sx++) {
                        float u = (px + (sx + 0.5f) / SS) * px1, v = (py + (sy + 0.5f) / SS) * px1;
                        if (d > 0 ? dil(s, u, v, d) : s.in(u, v)) hit++;
                    }
                }
                if (hit > 0) blend(ox + px, oy + py, argb, hit / (float) (SS * SS));
            }
        }
    }

    private void blend(int x, int y, int argb, float cov) {
        int i = y * aw + x;
        int dst = pixels[i];
        float sa = ((argb >>> 24) / 255f) * cov;
        float da = (dst >>> 24) / 255f;
        float oa = sa + da * (1 - sa);
        if (oa <= 0) return;
        int r = (int) ((((argb >> 16) & 255) * sa + ((dst >> 16) & 255) * da * (1 - sa)) / oa);
        int g = (int) ((((argb >> 8) & 255) * sa + ((dst >> 8) & 255) * da * (1 - sa)) / oa);
        int b = (int) (((argb & 255) * sa + (dst & 255) * da * (1 - sa)) / oa);
        pixels[i] = ((int) (oa * 255f + 0.5f) << 24) | (r << 16) | (g << 8) | b;
    }

    private void shape(Sh s, int argb) {
        fill(s, 0xFF000000, 0.055f);
        fill(s, argb, 0f);
    }

    private void drawIcon(char c, int x, int y, int size) {
        ox = x; oy = y; sz = size;
        switch (c) {
            case PICANHA: {
                Sh fat = new Ell(0.5f, 0.50f, 0.47f, 0.30f);
                Sh meat = new And(new Ell(0.5f, 0.62f, 0.45f, 0.22f), new Ell(0.5f, 0.50f, 0.47f, 0.30f));
                fill(new Union(fat, meat), 0xFF000000, 0.055f);
                fill(fat, 0xFFF4E2BE, 0);
                fill(meat, 0xFFC0362C, 0);
                fill(new And(new Union(new Seg(0.28f, 0.75f, 0.42f, 0.48f, 0.025f),
                        new Seg(0.46f, 0.80f, 0.60f, 0.48f, 0.025f),
                        new Seg(0.64f, 0.78f, 0.76f, 0.52f, 0.025f)), meat), 0xFF7A1D17, 0);
                break;
            }
            case MAGNET: {
                Sh ring = new And(new Minus(new Ell(0.5f, 0.48f, 0.37f, 0.37f), new Ell(0.5f, 0.48f, 0.15f, 0.15f)),
                        new Box(0, 0.48f, 1, 1));
                Sh bars = new Union(new Box(0.13f, 0.10f, 0.35f, 0.49f), new Box(0.65f, 0.10f, 0.87f, 0.49f));
                fill(new Union(ring, bars), 0xFF000000, 0.055f);
                fill(new Union(ring, bars), 0xFFE53935, 0);
                fill(new Union(new Box(0.13f, 0.10f, 0.35f, 0.26f), new Box(0.65f, 0.10f, 0.87f, 0.26f)), 0xFFDDE3EA, 0);
                break;
            }
            case PLANE: {
                Sh body = new Union(new Ell(0.5f, 0.5f, 0.075f, 0.42f),
                        new Poly(0.5f, 0.36f, 0.96f, 0.60f, 0.96f, 0.68f, 0.5f, 0.56f, 0.04f, 0.68f, 0.04f, 0.60f),
                        new Poly(0.5f, 0.76f, 0.74f, 0.88f, 0.74f, 0.94f, 0.5f, 0.88f, 0.26f, 0.94f, 0.26f, 0.88f));
                shape(body, 0xFFF5F7FA);
                fill(new Ell(0.5f, 0.2f, 0.04f, 0.07f), 0xFF2A6FD6, 0);
                break;
            }
            case SHOE: {
                Sh sole = new Union(new Ell(0.5f, 0.30f, 0.24f, 0.22f), new Ell(0.5f, 0.70f, 0.22f, 0.22f),
                        new Box(0.30f, 0.30f, 0.70f, 0.70f));
                shape(sole, 0xFF16A34A);
                fill(new Union(new Seg(0.5f, 0.24f, 0.31f, 0.58f, 0.045f), new Seg(0.5f, 0.24f, 0.69f, 0.58f, 0.045f)), 0xFFFFD60A, 0);
                break;
            }
            case PAUSE:
                shape(new Union(new Box(0.24f, 0.18f, 0.42f, 0.82f), new Box(0.58f, 0.18f, 0.76f, 0.82f)), 0xFFFFFFFF);
                break;
            case PLAY:
                shape(new Poly(0.28f, 0.16f, 0.84f, 0.5f, 0.28f, 0.84f), 0xFFFFFFFF);
                break;
            case SOUND:
            case MUTE: {
                Sh spk = new Union(new Box(0.12f, 0.38f, 0.32f, 0.62f), new Poly(0.30f, 0.38f, 0.55f, 0.16f, 0.55f, 0.84f, 0.30f, 0.62f));
                if (c == SOUND) {
                    Sh waves = new And(new Union(
                            new Minus(new Ell(0.55f, 0.5f, 0.22f, 0.22f), new Ell(0.55f, 0.5f, 0.15f, 0.15f)),
                            new Minus(new Ell(0.55f, 0.5f, 0.38f, 0.38f), new Ell(0.55f, 0.5f, 0.31f, 0.31f))),
                            new Box(0.62f, 0, 1, 1));
                    shape(new Union(spk, waves), 0xFFFFFFFF);
                } else {
                    Sh x1 = new Union(new Seg(0.65f, 0.36f, 0.90f, 0.64f, 0.05f), new Seg(0.65f, 0.64f, 0.90f, 0.36f, 0.05f));
                    shape(new Union(spk, x1), 0xFFFFFFFF);
                }
                break;
            }
            case STAR: {
                float[] p = new float[20];
                for (int i = 0; i < 10; i++) {
                    double a = -Math.PI / 2 + i * Math.PI / 5;
                    float r = (i % 2 == 0) ? 0.45f : 0.19f;
                    p[i * 2] = 0.5f + (float) Math.cos(a) * r;
                    p[i * 2 + 1] = 0.53f + (float) Math.sin(a) * r;
                }
                shape(new Poly(p), 0xFFFFD60A);
                break;
            }
            case DUCK: {
                Sh head = new Ell(0.45f, 0.48f, 0.33f, 0.33f);
                Sh beak = new Ell(0.80f, 0.58f, 0.17f, 0.09f);
                fill(new Union(head, beak), 0xFF000000, 0.055f);
                fill(head, 0xFFFFD000, 0);
                fill(beak, 0xFFFF8A00, 0);
                fill(new Ell(0.55f, 0.40f, 0.06f, 0.07f), 0xFF111111, 0);
                break;
            }
            case CIRCLE:
                fill(new Ell(0.5f, 0.5f, 0.47f, 0.47f), 0xFFFFFFFF, 0);
                break;
            case SOFT:
                for (int py = 0; py < sz; py++) {
                    for (int px = 0; px < sz; px++) {
                        float u = (px + 0.5f) / sz - 0.5f, v = (py + 0.5f) / sz - 0.5f;
                        float d = (float) Math.sqrt(u * u + v * v) * 2f;
                        if (d < 1f) {
                            float a = 1f - d;
                            a = a * a * (3 - 2 * a);
                            pixels[(oy + py) * aw + ox + px] = ((int) (a * 255) << 24) | 0xFFFFFF;
                        }
                    }
                }
                break;
            case LOCK: {
                Sh shackle = new And(new Minus(new Ell(0.5f, 0.42f, 0.24f, 0.26f), new Ell(0.5f, 0.42f, 0.13f, 0.15f)),
                        new Box(0, 0, 1, 0.5f));
                Sh legs = new Union(new Box(0.26f, 0.40f, 0.37f, 0.5f), new Box(0.63f, 0.40f, 0.74f, 0.5f));
                Sh body = new Box(0.18f, 0.48f, 0.82f, 0.88f);
                fill(new Union(shackle, legs, body), 0xFF000000, 0.055f);
                fill(new Union(shackle, legs), 0xFFB8C0CC, 0);
                fill(body, 0xFFF2B705, 0);
                fill(new Ell(0.5f, 0.64f, 0.06f, 0.07f), 0xFF5A4300, 0);
                break;
            }
            case CHECK:
                shape(new Union(new Seg(0.18f, 0.52f, 0.42f, 0.76f, 0.08f), new Seg(0.42f, 0.76f, 0.84f, 0.26f, 0.08f)), 0xFF22C55E);
                break;
            case UP:
                shape(new Union(new Poly(0.5f, 0.1f, 0.88f, 0.5f, 0.12f, 0.5f), new Box(0.36f, 0.48f, 0.64f, 0.9f)), 0xFFFFFFFF);
                break;
            case DOWN:
                shape(new Union(new Poly(0.5f, 0.9f, 0.12f, 0.5f, 0.88f, 0.5f), new Box(0.36f, 0.1f, 0.64f, 0.52f)), 0xFFFFFFFF);
                break;
            case LEFT:
                shape(new Union(new Poly(0.1f, 0.5f, 0.5f, 0.12f, 0.5f, 0.88f), new Box(0.48f, 0.36f, 0.9f, 0.64f)), 0xFFFFFFFF);
                break;
            case RIGHT:
                shape(new Union(new Poly(0.9f, 0.5f, 0.5f, 0.88f, 0.5f, 0.12f), new Box(0.1f, 0.36f, 0.52f, 0.64f)), 0xFFFFFFFF);
                break;
            case ROUND:
                fill(new Union(new Box(0.3f, 0.02f, 0.7f, 0.98f), new Box(0.02f, 0.3f, 0.98f, 0.7f),
                        new Ell(0.3f, 0.3f, 0.28f, 0.28f), new Ell(0.7f, 0.3f, 0.28f, 0.28f),
                        new Ell(0.3f, 0.7f, 0.28f, 0.28f), new Ell(0.7f, 0.7f, 0.28f, 0.28f)), 0xFFFFFFFF, 0);
                break;
            case WHITE:
                for (int py = 0; py < sz; py++)
                    for (int px = 0; px < sz; px++) pixels[(oy + py) * aw + ox + px] = 0xFFFFFFFF;
                break;
            case MULT: {
                Sh badge = new Ell(0.5f, 0.5f, 0.45f, 0.45f);
                shape(badge, 0xFFFFD60A);
                // "2" e "X" estilizados
                Sh two = new Union(new Seg(0.22f, 0.36f, 0.30f, 0.30f, 0.045f), new Seg(0.30f, 0.30f, 0.40f, 0.33f, 0.045f),
                        new Seg(0.40f, 0.33f, 0.42f, 0.44f, 0.045f), new Seg(0.42f, 0.44f, 0.22f, 0.68f, 0.045f),
                        new Seg(0.22f, 0.68f, 0.44f, 0.68f, 0.045f));
                Sh x2 = new Union(new Seg(0.54f, 0.38f, 0.78f, 0.68f, 0.05f), new Seg(0.54f, 0.68f, 0.78f, 0.38f, 0.05f));
                fill(new Union(two, x2), 0xFF0B6B2E, 0);
                break;
            }
            case CART: {
                Sh basket = new Poly(0.18f, 0.28f, 0.86f, 0.28f, 0.76f, 0.62f, 0.28f, 0.62f);
                Sh handle = new Seg(0.06f, 0.18f, 0.20f, 0.20f, 0.04f);
                Sh wheels = new Union(new Ell(0.34f, 0.78f, 0.08f, 0.08f), new Ell(0.70f, 0.78f, 0.08f, 0.08f));
                shape(new Union(basket, handle, wheels), 0xFFFFFFFF);
                break;
            }
            default:
                break;
        }
    }
}
