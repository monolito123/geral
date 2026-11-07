package com.monolito.correlula;

/** Monta geometria low-poly com iluminacao "assada" nas cores dos vertices. */
final class MeshBuilder {
    static final int TOP = 1, BOT = 2, FRONT = 4, BACK = 8, RIGHT = 16, LEFT = 32;
    static final int ALL = 63;
    /** Faces visiveis pela camera do jogo (que olha para -z, de cima). */
    static final int VIS = TOP | FRONT | RIGHT | LEFT;

    private static final float ST = 1.0f, SB = 0.55f, SF = 0.86f, SK = 0.66f, SR = 0.78f, SL = 0.70f;
    private static final int[][] FACES = {
            {6, 7, 3, 2}, // top
            {0, 1, 5, 4}, // bottom
            {4, 5, 7, 6}, // front (+z)
            {1, 0, 2, 3}, // back (-z)
            {5, 1, 3, 7}, // right (+x)
            {0, 4, 6, 2}, // left (-x)
    };

    float[] v = new float[6 * 4096];
    short[] ix = new short[6 * 4096];
    int nv, ni;

    private final float[] xf = new float[16];
    private boolean hasXf;
    private final float[] cx = new float[8], cy = new float[8], cz = new float[8];

    void reset() {
        nv = 0;
        ni = 0;
        hasXf = false;
    }

    void setTransform(float[] m) {
        System.arraycopy(m, 0, xf, 0, 16);
        hasXf = true;
    }

    void clearTransform() {
        hasXf = false;
    }

    private void ensure(int addV, int addI) {
        if ((nv + addV) * 6 > v.length) {
            float[] n = new float[Math.max(v.length * 2, (nv + addV) * 6)];
            System.arraycopy(v, 0, n, 0, nv * 6);
            v = n;
        }
        if (ni + addI > ix.length) {
            short[] n = new short[Math.max(ix.length * 2, ni + addI)];
            System.arraycopy(ix, 0, n, 0, ni);
            ix = n;
        }
    }

    private int vert(float x, float y, float z, float r, float g, float b) {
        int o = nv * 6;
        v[o] = x; v[o + 1] = y; v[o + 2] = z;
        v[o + 3] = r; v[o + 4] = g; v[o + 5] = b;
        return nv++;
    }

    private static float shade(float nx, float ny, float nz) {
        float l = (float) Math.sqrt(nx * nx + ny * ny + nz * nz);
        if (l < 1e-6f) return 1f;
        nx /= l; ny /= l; nz /= l;
        return nx * nx * (nx > 0 ? SR : SL) + ny * ny * (ny > 0 ? ST : SB) + nz * nz * (nz > 0 ? SF : SK);
    }

    void box(float x0, float y0, float z0, float x1, float y1, float z1, int rgb, int faces) {
        boxAO(x0, y0, z0, x1, y1, z1, rgb, faces, 1f);
    }

    /** Caixa alinhada; ao < 1 escurece os vertices de baixo (oclusao ambiente falsa). */
    void boxAO(float x0, float y0, float z0, float x1, float y1, float z1, int rgb, int faces, float ao) {
        for (int i = 0; i < 8; i++) {
            float x = (i & 1) != 0 ? x1 : x0;
            float y = (i & 2) != 0 ? y1 : y0;
            float z = (i & 4) != 0 ? z1 : z0;
            if (hasXf) {
                float[] m = xf;
                cx[i] = m[0] * x + m[4] * y + m[8] * z + m[12];
                cy[i] = m[1] * x + m[5] * y + m[9] * z + m[13];
                cz[i] = m[2] * x + m[6] * y + m[10] * z + m[14];
            } else {
                cx[i] = x; cy[i] = y; cz[i] = z;
            }
        }
        float r = ((rgb >> 16) & 255) / 255f, g = ((rgb >> 8) & 255) / 255f, b = (rgb & 255) / 255f;
        int cnt = Integer.bitCount(faces & ALL);
        ensure(cnt * 4, cnt * 6);
        for (int f = 0; f < 6; f++) {
            if ((faces & (1 << f)) == 0) continue;
            int[] q = FACES[f];
            int a = q[0], bb = q[1], c = q[2];
            float ux = cx[bb] - cx[a], uy = cy[bb] - cy[a], uz = cz[bb] - cz[a];
            float wx = cx[c] - cx[a], wy = cy[c] - cy[a], wz = cz[c] - cz[a];
            float s = shade(uy * wz - uz * wy, uz * wx - ux * wz, ux * wy - uy * wx);
            int base = nv;
            for (int k = 0; k < 4; k++) {
                int ci = q[k];
                float sh = s;
                if (ao != 1f && (ci & 2) == 0 && f != 0) sh *= ao;
                vert(cx[ci], cy[ci], cz[ci], r * sh, g * sh, b * sh);
            }
            ix[ni++] = (short) base; ix[ni++] = (short) (base + 1); ix[ni++] = (short) (base + 2);
            ix[ni++] = (short) base; ix[ni++] = (short) (base + 2); ix[ni++] = (short) (base + 3);
        }
    }

    /** Quad livre (vertices em ordem anti-horaria vista de fora). */
    void quad(float ax, float ay, float az, float bx, float by, float bz,
              float qx, float qy, float qz, float dx, float dy, float dz, int rgb) {
        float ux = bx - ax, uy = by - ay, uz = bz - az;
        float wx = qx - ax, wy = qy - ay, wz = qz - az;
        float s = shade(uy * wz - uz * wy, uz * wx - ux * wz, ux * wy - uy * wx);
        float r = ((rgb >> 16) & 255) / 255f * s, g = ((rgb >> 8) & 255) / 255f * s, b = (rgb & 255) / 255f * s;
        ensure(4, 6);
        int base = nv;
        vert(ax, ay, az, r, g, b);
        vert(bx, by, bz, r, g, b);
        vert(qx, qy, qz, r, g, b);
        vert(dx, dy, dz, r, g, b);
        ix[ni++] = (short) base; ix[ni++] = (short) (base + 1); ix[ni++] = (short) (base + 2);
        ix[ni++] = (short) base; ix[ni++] = (short) (base + 2); ix[ni++] = (short) (base + 3);
    }

    void tri(float ax, float ay, float az, float bx, float by, float bz,
             float qx, float qy, float qz, int rgb) {
        float ux = bx - ax, uy = by - ay, uz = bz - az;
        float wx = qx - ax, wy = qy - ay, wz = qz - az;
        float s = shade(uy * wz - uz * wy, uz * wx - ux * wz, ux * wy - uy * wx);
        float r = ((rgb >> 16) & 255) / 255f * s, g = ((rgb >> 8) & 255) / 255f * s, b = (rgb & 255) / 255f * s;
        ensure(3, 3);
        int base = nv;
        vert(ax, ay, az, r, g, b);
        vert(bx, by, bz, r, g, b);
        vert(qx, qy, qz, r, g, b);
        ix[ni++] = (short) base; ix[ni++] = (short) (base + 1); ix[ni++] = (short) (base + 2);
    }

    Gfx.Mesh build(Gfx g) {
        if (nv > 65535) throw new IllegalStateException("malha grande demais: " + nv);
        return g.createMesh(v, nv * 6, ix, ni);
    }
}
