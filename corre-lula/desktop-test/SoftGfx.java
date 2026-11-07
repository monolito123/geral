package com.monolito.correlula;

/** Rasterizador em software que imita o GLBackend, so para gerar previas no desktop. */
final class SoftGfx implements Gfx {
    final int W, H;
    final int[] color;
    final float[] depth;
    boolean dTest, dWrite, blend, cull;
    final float[] vp = new float[16];
    final float[] mvp = new float[16];
    float fog0, fogK, fr, fg, fb;
    int curTex;
    int[][] texPix = new int[8][];
    int[] texW = new int[8], texH = new int[8];
    int nTex;
    long tris;

    SoftGfx(int w, int h) {
        W = w;
        H = h;
        color = new int[w * h];
        depth = new float[w * h];
    }

    public Mesh createMesh(float[] v, int nFloats, short[] idx, int nIdx) {
        Mesh m = new Mesh();
        m.v = java.util.Arrays.copyOf(v, nFloats);
        m.idx = java.util.Arrays.copyOf(idx, nIdx);
        m.count = nIdx;
        return m;
    }

    public void deleteMesh(Mesh m) {}

    public int createTexture(int[] argb, int w, int h) {
        nTex++;
        texPix[nTex] = argb.clone();
        texW[nTex] = w;
        texH[nTex] = h;
        return nTex;
    }

    public void viewport(int w, int h) {}

    public void clear(float r, float g, float b) {
        int c = 0xFF000000 | ((int) (r * 255) << 16) | ((int) (g * 255) << 8) | (int) (b * 255);
        java.util.Arrays.fill(color, c);
        java.util.Arrays.fill(depth, 1f);
    }

    public void state(boolean depthTest, boolean depthWrite, boolean bl, boolean cl) {
        dTest = depthTest;
        dWrite = depthWrite;
        blend = bl;
        cull = cl;
    }

    private void fog(float start, float end, float r, float g, float b) {
        if (start >= end) { fog0 = 1e9f; fogK = 0; } else { fog0 = start; fogK = 1f / (end - start); }
        fr = r; fg = g; fb = b;
    }

    public void colorPass(float[] v, float fogStart, float fogEnd, float r, float g, float b) {
        System.arraycopy(v, 0, vp, 0, 16);
        fog(fogStart, fogEnd, r, g, b);
        curTex = 0;
    }

    public void texPass(float[] v, int tex, float fogStart, float fogEnd, float r, float g, float b) {
        System.arraycopy(v, 0, vp, 0, 16);
        fog(fogStart, fogEnd, r, g, b);
        curTex = tex;
    }

    // vertice de trabalho: x y z w | r g b a | u v | fog
    private static final int VS = 11;
    private final float[] tv = new float[VS * 3];
    private final float[] poly = new float[VS * 8];
    private final float[] poly2 = new float[VS * 8];

    public void drawMesh(Mesh m, float[] model, float r, float g, float b, float a) {
        Mat.mul(mvp, vp, model);
        float[] v = m.v;
        short[] ix = m.idx;
        for (int t = 0; t < m.count; t += 3) {
            for (int k = 0; k < 3; k++) {
                int vi = (ix[t + k] & 0xFFFF) * 6;
                float x = v[vi], y = v[vi + 1], z = v[vi + 2];
                int o = k * VS;
                xform(mvp, x, y, z, o);
                tv[o + 4] = v[vi + 3] * r;
                tv[o + 5] = v[vi + 4] * g;
                tv[o + 6] = v[vi + 5] * b;
                tv[o + 7] = a;
                tv[o + 8] = 0;
                tv[o + 9] = 0;
                tv[o + 10] = clamp((tv[o + 3] - fog0) * fogK);
            }
            tri(false);
        }
    }

    public void drawQuads(float[] d, int nq) {
        for (int q = 0; q < nq; q++) {
            int base = q * 36;
            for (int half = 0; half < 2; half++) {
                int[] ord = half == 0 ? new int[]{0, 1, 2} : new int[]{0, 2, 3};
                for (int k = 0; k < 3; k++) {
                    int o = base + ord[k] * 9;
                    int w = k * VS;
                    xform(vp, d[o], d[o + 1], d[o + 2], w);
                    tv[w + 4] = d[o + 5];
                    tv[w + 5] = d[o + 6];
                    tv[w + 6] = d[o + 7];
                    tv[w + 7] = d[o + 8];
                    tv[w + 8] = d[o + 3];
                    tv[w + 9] = d[o + 4];
                    tv[w + 10] = clamp((tv[w + 3] - fog0) * fogK);
                }
                tri(true);
            }
        }
    }

    private static float clamp(float f) {
        return f < 0 ? 0 : (f > 1 ? 1 : f);
    }

    private void xform(float[] m, float x, float y, float z, int o) {
        tv[o] = m[0] * x + m[4] * y + m[8] * z + m[12];
        tv[o + 1] = m[1] * x + m[5] * y + m[9] * z + m[13];
        tv[o + 2] = m[2] * x + m[6] * y + m[10] * z + m[14];
        tv[o + 3] = m[3] * x + m[7] * y + m[11] * z + m[15];
    }

    private void tri(boolean textured) {
        tris++;
        // recorte contra o plano near (z > -w)
        int n = 3;
        System.arraycopy(tv, 0, poly, 0, VS * 3);
        int out = 0;
        for (int i = 0; i < n; i++) {
            int a = i * VS, b = ((i + 1) % n) * VS;
            float da = poly[a + 2] + poly[a + 3], db = poly[b + 2] + poly[b + 3];
            if (da >= 0) { System.arraycopy(poly, a, poly2, out * VS, VS); out++; }
            if ((da >= 0) != (db >= 0)) {
                float t = da / (da - db);
                for (int k = 0; k < VS; k++) poly2[out * VS + k] = poly[a + k] + (poly[b + k] - poly[a + k]) * t;
                out++;
            }
        }
        if (out < 3) return;
        for (int i = 1; i + 1 < out; i++) raster(poly2, 0, i * VS, (i + 1) * VS, textured);
    }

    private final float[] sx = new float[3], sy = new float[3], sz = new float[3], iw = new float[3];

    private void raster(float[] p, int a, int b, int c, boolean textured) {
        int[] id = {a, b, c};
        for (int k = 0; k < 3; k++) {
            int o = id[k];
            float w = p[o + 3];
            if (w < 1e-5f) w = 1e-5f;
            iw[k] = 1f / w;
            sx[k] = (p[o] * iw[k] * 0.5f + 0.5f) * W;
            sy[k] = (1f - (p[o + 1] * iw[k] * 0.5f + 0.5f)) * H;
            sz[k] = p[o + 2] * iw[k] * 0.5f + 0.5f;
        }
        float area = (sx[1] - sx[0]) * (sy[2] - sy[0]) - (sx[2] - sx[0]) * (sy[1] - sy[0]);
        if (area == 0) return;
        if (cull && area > 0) return;
        int minX = Math.max(0, (int) Math.floor(Math.min(sx[0], Math.min(sx[1], sx[2]))));
        int maxX = Math.min(W - 1, (int) Math.ceil(Math.max(sx[0], Math.max(sx[1], sx[2]))));
        int minY = Math.max(0, (int) Math.floor(Math.min(sy[0], Math.min(sy[1], sy[2]))));
        int maxY = Math.min(H - 1, (int) Math.ceil(Math.max(sy[0], Math.max(sy[1], sy[2]))));
        float inv = 1f / area;
        int[] tex = textured ? texPix[curTex] : null;
        int tw = textured ? texW[curTex] : 0, th = textured ? texH[curTex] : 0;
        for (int y = minY; y <= maxY; y++) {
            float py = y + 0.5f;
            for (int x = minX; x <= maxX; x++) {
                float px = x + 0.5f;
                float w0 = ((sx[1] - px) * (sy[2] - py) - (sx[2] - px) * (sy[1] - py)) * inv;
                float w1 = ((sx[2] - px) * (sy[0] - py) - (sx[0] - px) * (sy[2] - py)) * inv;
                float w2 = 1f - w0 - w1;
                if (w0 < 0 || w1 < 0 || w2 < 0) continue;
                float z = w0 * sz[0] + w1 * sz[1] + w2 * sz[2];
                int pi = y * W + x;
                if (dTest && z > depth[pi]) continue;
                float q0 = w0 * iw[0], q1 = w1 * iw[1], q2 = w2 * iw[2];
                float qs = 1f / (q0 + q1 + q2);
                q0 *= qs; q1 *= qs; q2 *= qs;
                float r = q0 * p[a + 4] + q1 * p[b + 4] + q2 * p[c + 4];
                float g = q0 * p[a + 5] + q1 * p[b + 5] + q2 * p[c + 5];
                float bl = q0 * p[a + 6] + q1 * p[b + 6] + q2 * p[c + 6];
                float al = q0 * p[a + 7] + q1 * p[b + 7] + q2 * p[c + 7];
                float fogF = q0 * p[a + 10] + q1 * p[b + 10] + q2 * p[c + 10];
                if (tex != null) {
                    float u = q0 * p[a + 8] + q1 * p[b + 8] + q2 * p[c + 8];
                    float v = q0 * p[a + 9] + q1 * p[b + 9] + q2 * p[c + 9];
                    int t = sample(tex, tw, th, u, v);
                    al *= (t >>> 24) / 255f;
                    r *= ((t >> 16) & 255) / 255f;
                    g *= ((t >> 8) & 255) / 255f;
                    bl *= (t & 255) / 255f;
                }
                r = r + (fr - r) * fogF;
                g = g + (fg - g) * fogF;
                bl = bl + (fb - bl) * fogF;
                if (blend) {
                    if (al <= 0.003f) continue;
                    int d = color[pi];
                    float dr = ((d >> 16) & 255) / 255f, dg = ((d >> 8) & 255) / 255f, db = (d & 255) / 255f;
                    r = r * al + dr * (1 - al);
                    g = g * al + dg * (1 - al);
                    bl = bl * al + db * (1 - al);
                }
                color[pi] = 0xFF000000 | (c255(r) << 16) | (c255(g) << 8) | c255(bl);
                if (dWrite) depth[pi] = z;
            }
        }
    }

    private static int c255(float f) {
        int v = (int) (f * 255f + 0.5f);
        return v < 0 ? 0 : (v > 255 ? 255 : v);
    }

    private static int sample(int[] t, int w, int h, float u, float v) {
        float fx = u * w - 0.5f, fy = v * h - 0.5f;
        int x0 = (int) Math.floor(fx), y0 = (int) Math.floor(fy);
        float ax = fx - x0, ay = fy - y0;
        int c00 = px(t, w, h, x0, y0), c10 = px(t, w, h, x0 + 1, y0), c01 = px(t, w, h, x0, y0 + 1), c11 = px(t, w, h, x0 + 1, y0 + 1);
        int res = 0;
        for (int sh = 0; sh < 32; sh += 8) {
            float a = ((c00 >>> sh) & 255) * (1 - ax) + ((c10 >>> sh) & 255) * ax;
            float b = ((c01 >>> sh) & 255) * (1 - ax) + ((c11 >>> sh) & 255) * ax;
            int val = (int) (a * (1 - ay) + b * ay + 0.5f);
            res |= (val & 255) << sh;
        }
        return res;
    }

    private static int px(int[] t, int w, int h, int x, int y) {
        x = x < 0 ? 0 : (x >= w ? w - 1 : x);
        y = y < 0 ? 0 : (y >= h ? h - 1 : y);
        return t[y * w + x];
    }

    void save(String path) throws java.io.IOException {
        java.awt.image.BufferedImage img = new java.awt.image.BufferedImage(W, H, java.awt.image.BufferedImage.TYPE_INT_RGB);
        img.setRGB(0, 0, W, H, color, 0, W);
        javax.imageio.ImageIO.write(img, "png", new java.io.File(path));
    }
}
