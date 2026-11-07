package com.monolito.correlula;

/** Agrupa quads texturizados (texto, icones, sprites) em poucas chamadas de desenho. */
final class Batch {
    static final int MAXQ = 1500;
    static final int LEFT = 0, CENTER = 1, RIGHT = 2;

    final float[] buf = new float[MAXQ * 36];
    int nq;
    final Gfx g;
    final Font f;

    Batch(Gfx g, Font f) {
        this.g = g;
        this.f = f;
    }

    void flush() {
        if (nq > 0) {
            g.drawQuads(buf, nq);
            nq = 0;
        }
    }

    private int put(int o, float x, float y, float z, float u, float v, float r, float gg, float b, float a) {
        float[] d = buf;
        d[o] = x; d[o + 1] = y; d[o + 2] = z; d[o + 3] = u; d[o + 4] = v;
        d[o + 5] = r; d[o + 6] = gg; d[o + 7] = b; d[o + 8] = a;
        return o + 9;
    }

    /** Quad arbitrario: a,b,c,d em sequencia. */
    void quad(float ax, float ay, float az, float bx, float by, float bz,
              float cx, float cy, float cz, float dx, float dy, float dz,
              float u0, float v0, float u1, float v1, int argb) {
        if (nq >= MAXQ) flush();
        float a = (argb >>> 24) / 255f, r = ((argb >> 16) & 255) / 255f, gg = ((argb >> 8) & 255) / 255f, b = (argb & 255) / 255f;
        int o = nq * 36;
        o = put(o, ax, ay, az, u0, v0, r, gg, b, a);
        o = put(o, bx, by, bz, u1, v0, r, gg, b, a);
        o = put(o, cx, cy, cz, u1, v1, r, gg, b, a);
        put(o, dx, dy, dz, u0, v1, r, gg, b, a);
        nq++;
    }

    /** Retangulo 2D (y para baixo) com regiao da textura. */
    void rect(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) {
        quad(x0, y0, 0, x1, y0, 0, x1, y1, 0, x0, y1, 0, u0, v0, u1, v1, argb);
    }

    void fillRect(float x, float y, float w, float h, int argb) {
        char c = Font.WHITE;
        rect(x, y, x + w, y + h, f.u0[c], f.v0[c], f.u1[c], f.v1[c], argb);
    }

    void gradient(float x, float y, float w, float h, int top, int bottom) {
        if (nq >= MAXQ) flush();
        char c = Font.WHITE;
        float u = f.u0[c], v = f.v0[c];
        int o = nq * 36;
        float a0 = (top >>> 24) / 255f, r0 = ((top >> 16) & 255) / 255f, g0 = ((top >> 8) & 255) / 255f, b0 = (top & 255) / 255f;
        float a1 = (bottom >>> 24) / 255f, r1 = ((bottom >> 16) & 255) / 255f, g1 = ((bottom >> 8) & 255) / 255f, b1 = (bottom & 255) / 255f;
        o = put(o, x, y, 0, u, v, r0, g0, b0, a0);
        o = put(o, x + w, y, 0, u, v, r0, g0, b0, a0);
        o = put(o, x + w, y + h, 0, u, v, r1, g1, b1, a1);
        put(o, x, y + h, 0, u, v, r1, g1, b1, a1);
        nq++;
    }

    /** Icone/sprite 2D centrado. */
    void icon(char c, float cx, float cy, float size, int argb) {
        float h = size * 0.5f;
        rect(cx - h, cy - h, cx + h, cy + h, f.u0[c], f.v0[c], f.u1[c], f.v1[c], argb);
    }

    /** Retangulo arredondado (9 fatias do icone ROUND). */
    void round(float x, float y, float w, float h, float r, int argb) {
        char c = Font.ROUND;
        float uA = f.u0[c], uD = f.u1[c], vA = f.v0[c], vD = f.v1[c];
        float uB = uA + (uD - uA) * 0.32f, uC = uD - (uD - uA) * 0.32f;
        float vB = vA + (vD - vA) * 0.32f, vC = vD - (vD - vA) * 0.32f;
        if (r * 2 > w) r = w * 0.5f;
        if (r * 2 > h) r = h * 0.5f;
        float x1 = x + r, x2 = x + w - r, x3 = x + w;
        float y1 = y + r, y2 = y + h - r, y3 = y + h;
        rect(x, y, x1, y1, uA, vA, uB, vB, argb);
        rect(x1, y, x2, y1, uB, vA, uC, vB, argb);
        rect(x2, y, x3, y1, uC, vA, uD, vB, argb);
        rect(x, y1, x1, y2, uA, vB, uB, vC, argb);
        rect(x1, y1, x2, y2, uB, vB, uC, vC, argb);
        rect(x2, y1, x3, y2, uC, vB, uD, vC, argb);
        rect(x, y2, x1, y3, uA, vC, uB, vD, argb);
        rect(x1, y2, x2, y3, uB, vC, uC, vD, argb);
        rect(x2, y2, x3, y3, uC, vC, uD, vD, argb);
    }

    float textWidth(CharSequence s, float size) {
        return f.width(s) * size / f.cellH;
    }

    /** Texto 2D numa linha. y = centro vertical. Retorna a largura. */
    float text(CharSequence s, float x, float y, float size, int argb, int align) {
        return text(s, 0, s.length(), x, y, size, argb, align);
    }

    float text(CharSequence s, int from, int to, float x, float y, float size, int argb, int align) {
        float sc = size / f.cellH;
        float w = f.width(s, from, to) * sc;
        float pen = align == CENTER ? x - w * 0.5f : (align == RIGHT ? x - w : x);
        float top = y - size * 0.5f, bot = y + size * 0.5f;
        float sp = Font.STROKE * 0.55f;
        for (int i = from; i < to; i++) {
            char c = s.charAt(i);
            if (c >= 256 || f.cw[c] <= 0) continue;
            float gx = pen - f.offX[c] * sc;
            if (c != ' ') rect(gx, top, gx + f.cw[c] * sc, bot, f.u0[c], f.v0[c], f.u1[c], f.v1[c], argb);
            pen += (f.adv[c] + sp) * sc;
        }
        return w;
    }

    /** Texto que encolhe para caber em maxW. */
    float textFit(CharSequence s, float x, float y, float size, float maxW, int argb, int align) {
        return textFit(s, 0, s.length(), x, y, size, maxW, argb, align);
    }

    float textFit(CharSequence s, int from, int to, float x, float y, float size, float maxW, int argb, int align) {
        float w = f.width(s, from, to) * size / f.cellH;
        if (w > maxW) size *= maxW / w;
        return text(s, from, to, x, y, size, argb, align);
    }

    /** Largura da linha mais larga de um texto com '\n'. */
    float linesWidth(CharSequence s, float size) {
        float m = 0;
        int st = 0, n = s.length();
        for (int i = 0; i <= n; i++) {
            if (i == n || s.charAt(i) == '\n') {
                m = Math.max(m, f.width(s, st, i) * size / f.cellH);
                st = i + 1;
            }
        }
        return m;
    }

    static int lineCount(CharSequence s) {
        int lines = 1;
        for (int i = 0; i < s.length(); i++) if (s.charAt(i) == '\n') lines++;
        return lines;
    }

    /** Texto com varias linhas separadas por '\n'; y = centro do bloco. */
    void textLines(CharSequence s, float x, float y, float size, float maxW, int argb) {
        int lines = lineCount(s);
        float lh = size * 0.92f;
        float yy = y - (lines - 1) * lh * 0.5f;
        int st = 0, n = s.length();
        for (int i = 0; i <= n; i++) {
            if (i == n || s.charAt(i) == '\n') {
                textFit(s, st, i, x, yy, size, maxW, argb, CENTER);
                yy += lh;
                st = i + 1;
            }
        }
    }

    /** Texto 3D no plano XY (virado para +z), centrado em x e y. */
    void text3D(CharSequence s, int from, int to, float x, float y, float z, float size, int argb) {
        float sc = size / f.cellH;
        float w = 0;
        float sp = Font.STROKE * 0.55f;
        for (int i = from; i < to; i++) {
            char c = s.charAt(i);
            if (c < 256 && f.cw[c] > 0) w += f.adv[c] + sp;
        }
        w *= sc;
        float pen = x - w * 0.5f;
        float top = y + size * 0.5f, bot = y - size * 0.5f;
        for (int i = from; i < to; i++) {
            char c = s.charAt(i);
            if (c >= 256 || f.cw[c] <= 0) continue;
            float gx = pen - f.offX[c] * sc;
            if (c != ' ') {
                float gx1 = gx + f.cw[c] * sc;
                quad(gx, top, z, gx1, top, z, gx1, bot, z, gx, bot, z, f.u0[c], f.v0[c], f.u1[c], f.v1[c], argb);
            }
            pen += (f.adv[c] + sp) * sc;
        }
    }

    /** Largura de um trecho em unidades 3D. */
    float width3D(CharSequence s, int from, int to, float size) {
        float sc = size / f.cellH, w = 0, sp = Font.STROKE * 0.55f;
        for (int i = from; i < to; i++) {
            char c = s.charAt(i);
            if (c < 256 && f.cw[c] > 0) w += f.adv[c] + sp;
        }
        return w * sc;
    }

    /** Texto 3D multi-linha ajustado para caber numa placa w x h. */
    void sign3D(String s, float x, float y, float z, float w, float h, int argb) {
        int lines = 1;
        for (int i = 0; i < s.length(); i++) if (s.charAt(i) == '\n') lines++;
        float size = h / (lines * 0.95f + 0.1f);
        // limita pela linha mais larga
        int st = 0;
        float maxW = 0;
        for (int i = 0; i <= s.length(); i++) {
            if (i == s.length() || s.charAt(i) == '\n') {
                maxW = Math.max(maxW, width3D(s, st, i, size));
                st = i + 1;
            }
        }
        if (maxW > w) size *= w / maxW;
        float lh = size * 0.95f;
        float yy = y + (lines - 1) * lh * 0.5f;
        st = 0;
        for (int i = 0; i <= s.length(); i++) {
            if (i == s.length() || s.charAt(i) == '\n') {
                text3D(s, st, i, x, yy, z, size, argb);
                yy -= lh;
                st = i + 1;
            }
        }
    }

    /** Sprite 3D virado para a camera (plano XY). */
    void sprite3D(char c, float x, float y, float z, float size, int argb) {
        float h = size * 0.5f;
        quad(x - h, y + h, z, x + h, y + h, z, x + h, y - h, z, x - h, y - h, z, f.u0[c], f.v0[c], f.u1[c], f.v1[c], argb);
    }

    /** Sprite deitado no chao (plano XZ). */
    void ground3D(char c, float x, float y, float z, float sx, float sz, int argb) {
        quad(x - sx, y, z - sz, x + sx, y, z - sz, x + sx, y, z + sz, x - sx, y, z + sz, f.u0[c], f.v0[c], f.u1[c], f.v1[c], argb);
    }
}
