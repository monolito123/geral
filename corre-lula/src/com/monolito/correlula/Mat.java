package com.monolito.correlula;

/** Matrizes 4x4 column-major (padrao OpenGL), sem alocacao. */
final class Mat {
    private Mat() {}

    static void identity(float[] m) {
        for (int i = 0; i < 16; i++) m[i] = 0f;
        m[0] = m[5] = m[10] = m[15] = 1f;
    }

    static void copy(float[] dst, float[] src) {
        System.arraycopy(src, 0, dst, 0, 16);
    }

    /** out = a * b. out nao pode ser a nem b. */
    static void mul(float[] out, float[] a, float[] b) {
        for (int c = 0; c < 4; c++) {
            int c4 = c * 4;
            float b0 = b[c4], b1 = b[c4 + 1], b2 = b[c4 + 2], b3 = b[c4 + 3];
            out[c4] = a[0] * b0 + a[4] * b1 + a[8] * b2 + a[12] * b3;
            out[c4 + 1] = a[1] * b0 + a[5] * b1 + a[9] * b2 + a[13] * b3;
            out[c4 + 2] = a[2] * b0 + a[6] * b1 + a[10] * b2 + a[14] * b3;
            out[c4 + 3] = a[3] * b0 + a[7] * b1 + a[11] * b2 + a[15] * b3;
        }
    }

    /** m = m * T(x,y,z) */
    static void translate(float[] m, float x, float y, float z) {
        m[12] += m[0] * x + m[4] * y + m[8] * z;
        m[13] += m[1] * x + m[5] * y + m[9] * z;
        m[14] += m[2] * x + m[6] * y + m[10] * z;
        m[15] += m[3] * x + m[7] * y + m[11] * z;
    }

    static void scale(float[] m, float x, float y, float z) {
        for (int i = 0; i < 4; i++) {
            m[i] *= x;
            m[4 + i] *= y;
            m[8 + i] *= z;
        }
    }

    static void rotX(float[] m, float a) {
        if (a == 0f) return;
        float c = (float) Math.cos(a), s = (float) Math.sin(a);
        for (int i = 0; i < 4; i++) {
            float y = m[4 + i], z = m[8 + i];
            m[4 + i] = y * c + z * s;
            m[8 + i] = -y * s + z * c;
        }
    }

    static void rotY(float[] m, float a) {
        if (a == 0f) return;
        float c = (float) Math.cos(a), s = (float) Math.sin(a);
        for (int i = 0; i < 4; i++) {
            float x = m[i], z = m[8 + i];
            m[i] = x * c - z * s;
            m[8 + i] = x * s + z * c;
        }
    }

    static void rotZ(float[] m, float a) {
        if (a == 0f) return;
        float c = (float) Math.cos(a), s = (float) Math.sin(a);
        for (int i = 0; i < 4; i++) {
            float x = m[i], y = m[4 + i];
            m[i] = x * c + y * s;
            m[4 + i] = -x * s + y * c;
        }
    }

    static void perspective(float[] m, float fovyRad, float aspect, float n, float f) {
        float t = 1f / (float) Math.tan(fovyRad * 0.5f);
        for (int i = 0; i < 16; i++) m[i] = 0f;
        m[0] = t / aspect;
        m[5] = t;
        m[10] = (f + n) / (n - f);
        m[11] = -1f;
        m[14] = 2f * f * n / (n - f);
    }

    static void ortho(float[] m, float l, float r, float b, float t, float n, float f) {
        for (int i = 0; i < 16; i++) m[i] = 0f;
        m[0] = 2f / (r - l);
        m[5] = 2f / (t - b);
        m[10] = -2f / (f - n);
        m[12] = -(r + l) / (r - l);
        m[13] = -(t + b) / (t - b);
        m[14] = -(f + n) / (f - n);
        m[15] = 1f;
    }

    static void lookAt(float[] m, float ex, float ey, float ez, float cx, float cy, float cz) {
        float fx = cx - ex, fy = cy - ey, fz = cz - ez;
        float rl = 1f / (float) Math.sqrt(fx * fx + fy * fy + fz * fz);
        fx *= rl; fy *= rl; fz *= rl;
        // s = f x up(0,1,0)
        float sx = -fz, sy = 0f, sz = fx;
        rl = 1f / (float) Math.sqrt(sx * sx + sz * sz);
        sx *= rl; sz *= rl;
        // u = s x f
        float ux = sy * fz - sz * fy, uy = sz * fx - sx * fz, uz = sx * fy - sy * fx;
        m[0] = sx; m[4] = sy; m[8] = sz;
        m[1] = ux; m[5] = uy; m[9] = uz;
        m[2] = -fx; m[6] = -fy; m[10] = -fz;
        m[3] = 0f; m[7] = 0f; m[11] = 0f;
        m[12] = -(sx * ex + sy * ey + sz * ez);
        m[13] = -(ux * ex + uy * ey + uz * ez);
        m[14] = (fx * ex + fy * ey + fz * ez);
        m[15] = 1f;
    }
}
