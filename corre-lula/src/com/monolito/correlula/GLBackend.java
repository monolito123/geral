package com.monolito.correlula;

import android.opengl.GLES20;
import android.util.Log;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.FloatBuffer;
import java.nio.ShortBuffer;

/** Implementacao OpenGL ES 2.0 da camada grafica. */
final class GLBackend implements Gfx {
    private static final String VS_C =
            "uniform mat4 uVP;\n" +
            "uniform mat4 uM;\n" +
            "uniform vec4 uTint;\n" +
            "uniform vec2 uFog;\n" +
            "attribute vec3 aPos;\n" +
            "attribute vec3 aCol;\n" +
            "varying vec4 vCol;\n" +
            "varying float vFog;\n" +
            "void main() {\n" +
            "  vec4 p = uVP * (uM * vec4(aPos, 1.0));\n" +
            "  gl_Position = p;\n" +
            "  vCol = vec4(aCol * uTint.rgb, uTint.a);\n" +
            "  vFog = clamp((p.w - uFog.x) * uFog.y, 0.0, 1.0);\n" +
            "}\n";
    private static final String FS_C =
            "precision mediump float;\n" +
            "uniform vec3 uFogCol;\n" +
            "varying vec4 vCol;\n" +
            "varying float vFog;\n" +
            "void main() {\n" +
            "  gl_FragColor = vec4(mix(vCol.rgb, uFogCol, vFog), vCol.a);\n" +
            "}\n";
    private static final String VS_T =
            "uniform mat4 uVP;\n" +
            "uniform vec2 uFog;\n" +
            "attribute vec3 aPos;\n" +
            "attribute vec2 aUV;\n" +
            "attribute vec4 aCol;\n" +
            "varying vec2 vUV;\n" +
            "varying vec4 vCol;\n" +
            "varying float vFog;\n" +
            "void main() {\n" +
            "  vec4 p = uVP * vec4(aPos, 1.0);\n" +
            "  gl_Position = p;\n" +
            "  vUV = aUV;\n" +
            "  vCol = aCol;\n" +
            "  vFog = clamp((p.w - uFog.x) * uFog.y, 0.0, 1.0);\n" +
            "}\n";
    private static final String FS_T =
            "precision mediump float;\n" +
            "uniform sampler2D uTex;\n" +
            "uniform vec3 uFogCol;\n" +
            "varying vec2 vUV;\n" +
            "varying vec4 vCol;\n" +
            "varying float vFog;\n" +
            "void main() {\n" +
            "  vec4 t = texture2D(uTex, vUV) * vCol;\n" +
            "  gl_FragColor = vec4(mix(t.rgb, uFogCol, vFog), t.a);\n" +
            "}\n";

    private static final int A_POS = 0, A_COL = 1, A_UV = 2;

    private int progC, progT;
    private int cVP, cM, cTint, cFog, cFogCol;
    private int tVP, tTex, tFog, tFogCol;
    private int quadIbo;
    private FloatBuffer qfb;
    private FloatBuffer tmpF;
    private ShortBuffer tmpS;
    private Mesh bound;
    private int curProg = -1;
    private int sDepth = -1, sWrite = -1, sBlend = -1, sCull = -1;

    void init() {
        progC = link(VS_C, FS_C, false);
        progT = link(VS_T, FS_T, true);
        cVP = GLES20.glGetUniformLocation(progC, "uVP");
        cM = GLES20.glGetUniformLocation(progC, "uM");
        cTint = GLES20.glGetUniformLocation(progC, "uTint");
        cFog = GLES20.glGetUniformLocation(progC, "uFog");
        cFogCol = GLES20.glGetUniformLocation(progC, "uFogCol");
        tVP = GLES20.glGetUniformLocation(progT, "uVP");
        tTex = GLES20.glGetUniformLocation(progT, "uTex");
        tFog = GLES20.glGetUniformLocation(progT, "uFog");
        tFogCol = GLES20.glGetUniformLocation(progT, "uFogCol");

        short[] idx = new short[Batch.MAXQ * 6];
        for (int q = 0; q < Batch.MAXQ; q++) {
            int b = q * 4, o = q * 6;
            idx[o] = (short) b; idx[o + 1] = (short) (b + 1); idx[o + 2] = (short) (b + 2);
            idx[o + 3] = (short) b; idx[o + 4] = (short) (b + 2); idx[o + 5] = (short) (b + 3);
        }
        int[] id = new int[1];
        GLES20.glGenBuffers(1, id, 0);
        quadIbo = id[0];
        ShortBuffer sb = ByteBuffer.allocateDirect(idx.length * 2).order(ByteOrder.nativeOrder()).asShortBuffer();
        sb.put(idx).position(0);
        GLES20.glBindBuffer(GLES20.GL_ELEMENT_ARRAY_BUFFER, quadIbo);
        GLES20.glBufferData(GLES20.GL_ELEMENT_ARRAY_BUFFER, idx.length * 2, sb, GLES20.GL_STATIC_DRAW);
        qfb = ByteBuffer.allocateDirect(Batch.MAXQ * 36 * 4).order(ByteOrder.nativeOrder()).asFloatBuffer();

        GLES20.glDepthFunc(GLES20.GL_LEQUAL);
        GLES20.glCullFace(GLES20.GL_BACK);
        GLES20.glFrontFace(GLES20.GL_CCW);
        GLES20.glBlendFunc(GLES20.GL_SRC_ALPHA, GLES20.GL_ONE_MINUS_SRC_ALPHA);
        GLES20.glEnableVertexAttribArray(A_POS);
        GLES20.glEnableVertexAttribArray(A_COL);
    }

    private static int shader(int type, String src) {
        int s = GLES20.glCreateShader(type);
        GLES20.glShaderSource(s, src);
        GLES20.glCompileShader(s);
        int[] ok = new int[1];
        GLES20.glGetShaderiv(s, GLES20.GL_COMPILE_STATUS, ok, 0);
        if (ok[0] == 0) {
            String log = GLES20.glGetShaderInfoLog(s);
            Log.e("CorreLula", "shader: " + log);
            throw new RuntimeException("Erro no shader: " + log);
        }
        return s;
    }

    private static int link(String vs, String fs, boolean uv) {
        int p = GLES20.glCreateProgram();
        GLES20.glAttachShader(p, shader(GLES20.GL_VERTEX_SHADER, vs));
        GLES20.glAttachShader(p, shader(GLES20.GL_FRAGMENT_SHADER, fs));
        GLES20.glBindAttribLocation(p, A_POS, "aPos");
        GLES20.glBindAttribLocation(p, A_COL, "aCol");
        if (uv) GLES20.glBindAttribLocation(p, A_UV, "aUV");
        GLES20.glLinkProgram(p);
        int[] ok = new int[1];
        GLES20.glGetProgramiv(p, GLES20.GL_LINK_STATUS, ok, 0);
        if (ok[0] == 0) {
            String log = GLES20.glGetProgramInfoLog(p);
            Log.e("CorreLula", "link: " + log);
            throw new RuntimeException("Erro no programa: " + log);
        }
        return p;
    }

    private FloatBuffer floats(float[] v, int n) {
        if (tmpF == null || tmpF.capacity() < n) {
            tmpF = ByteBuffer.allocateDirect(Math.max(n, 16384) * 4).order(ByteOrder.nativeOrder()).asFloatBuffer();
        }
        tmpF.clear();
        tmpF.put(v, 0, n);
        tmpF.position(0);
        return tmpF;
    }

    private ShortBuffer shorts(short[] v, int n) {
        if (tmpS == null || tmpS.capacity() < n) {
            tmpS = ByteBuffer.allocateDirect(Math.max(n, 16384) * 2).order(ByteOrder.nativeOrder()).asShortBuffer();
        }
        tmpS.clear();
        tmpS.put(v, 0, n);
        tmpS.position(0);
        return tmpS;
    }

    @Override
    public Mesh createMesh(float[] v, int nFloats, short[] idx, int nIdx) {
        Mesh m = new Mesh();
        int[] ids = new int[2];
        GLES20.glGenBuffers(2, ids, 0);
        m.vbo = ids[0];
        m.ibo = ids[1];
        m.count = nIdx;
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, m.vbo);
        GLES20.glBufferData(GLES20.GL_ARRAY_BUFFER, nFloats * 4, floats(v, nFloats), GLES20.GL_STATIC_DRAW);
        GLES20.glBindBuffer(GLES20.GL_ELEMENT_ARRAY_BUFFER, m.ibo);
        GLES20.glBufferData(GLES20.GL_ELEMENT_ARRAY_BUFFER, nIdx * 2, shorts(idx, nIdx), GLES20.GL_STATIC_DRAW);
        bound = null;
        return m;
    }

    @Override
    public void deleteMesh(Mesh m) {
        if (m == null || m.vbo == 0) return;
        int[] ids = {m.vbo, m.ibo};
        GLES20.glDeleteBuffers(2, ids, 0);
        m.vbo = 0;
        m.ibo = 0;
        if (bound == m) bound = null;
    }

    @Override
    public int createTexture(int[] argb, int w, int h) {
        // ARGB -> bytes R,G,B,A na memoria (conversao em lote, rapida)
        int n = w * h;
        int[] rgba = new int[n];
        boolean le = ByteOrder.nativeOrder() == ByteOrder.LITTLE_ENDIAN;
        for (int i = 0; i < n; i++) {
            int c = argb[i];
            rgba[i] = le ? ((c & 0xFF00FF00) | ((c >> 16) & 0xFF) | ((c & 0xFF) << 16)) : ((c << 8) | (c >>> 24));
        }
        ByteBuffer bb = ByteBuffer.allocateDirect(n * 4).order(ByteOrder.nativeOrder());
        bb.asIntBuffer().put(rgba);
        bb.position(0);
        int[] id = new int[1];
        GLES20.glGenTextures(1, id, 0);
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, id[0]);
        GLES20.glPixelStorei(GLES20.GL_UNPACK_ALIGNMENT, 4);
        GLES20.glTexImage2D(GLES20.GL_TEXTURE_2D, 0, GLES20.GL_RGBA, w, h, 0, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, bb);
        GLES20.glGenerateMipmap(GLES20.GL_TEXTURE_2D);
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR_MIPMAP_LINEAR);
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR);
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE);
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE);
        return id[0];
    }

    @Override
    public void viewport(int w, int h) {
        GLES20.glViewport(0, 0, w, h);
    }

    @Override
    public void clear(float r, float g, float b) {
        GLES20.glDepthMask(true);
        sWrite = 1;
        GLES20.glClearColor(r, g, b, 1f);
        GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT | GLES20.GL_DEPTH_BUFFER_BIT);
    }

    @Override
    public void state(boolean depthTest, boolean depthWrite, boolean blend, boolean cull) {
        int d = depthTest ? 1 : 0, w = depthWrite ? 1 : 0, bl = blend ? 1 : 0, c = cull ? 1 : 0;
        if (d != sDepth) { if (depthTest) GLES20.glEnable(GLES20.GL_DEPTH_TEST); else GLES20.glDisable(GLES20.GL_DEPTH_TEST); sDepth = d; }
        if (w != sWrite) { GLES20.glDepthMask(depthWrite); sWrite = w; }
        if (bl != sBlend) { if (blend) GLES20.glEnable(GLES20.GL_BLEND); else GLES20.glDisable(GLES20.GL_BLEND); sBlend = bl; }
        if (c != sCull) { if (cull) GLES20.glEnable(GLES20.GL_CULL_FACE); else GLES20.glDisable(GLES20.GL_CULL_FACE); sCull = c; }
    }

    @Override
    public void colorPass(float[] vp, float fogStart, float fogEnd, float fr, float fg, float fb) {
        if (curProg != progC) {
            GLES20.glUseProgram(progC);
            GLES20.glDisableVertexAttribArray(A_UV);
            curProg = progC;
            bound = null;
        }
        GLES20.glUniformMatrix4fv(cVP, 1, false, vp, 0);
        if (fogStart >= fogEnd) GLES20.glUniform2f(cFog, 1e6f, 0f);
        else GLES20.glUniform2f(cFog, fogStart, 1f / (fogEnd - fogStart));
        GLES20.glUniform3f(cFogCol, fr, fg, fb);
    }

    @Override
    public void drawMesh(Mesh m, float[] model, float r, float g, float b, float a) {
        if (m == null || m.vbo == 0) return;
        if (bound != m) {
            GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, m.vbo);
            GLES20.glVertexAttribPointer(A_POS, 3, GLES20.GL_FLOAT, false, 24, 0);
            GLES20.glVertexAttribPointer(A_COL, 3, GLES20.GL_FLOAT, false, 24, 12);
            GLES20.glBindBuffer(GLES20.GL_ELEMENT_ARRAY_BUFFER, m.ibo);
            bound = m;
        }
        GLES20.glUniformMatrix4fv(cM, 1, false, model, 0);
        GLES20.glUniform4f(cTint, r, g, b, a);
        GLES20.glDrawElements(GLES20.GL_TRIANGLES, m.count, GLES20.GL_UNSIGNED_SHORT, 0);
    }

    @Override
    public void texPass(float[] vp, int tex, float fogStart, float fogEnd, float fr, float fg, float fb) {
        if (curProg != progT) {
            GLES20.glUseProgram(progT);
            GLES20.glEnableVertexAttribArray(A_UV);
            GLES20.glUniform1i(tTex, 0);
            curProg = progT;
        }
        bound = null;
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0);
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, tex);
        GLES20.glUniformMatrix4fv(tVP, 1, false, vp, 0);
        if (fogStart >= fogEnd) GLES20.glUniform2f(tFog, 1e6f, 0f);
        else GLES20.glUniform2f(tFog, fogStart, 1f / (fogEnd - fogStart));
        GLES20.glUniform3f(tFogCol, fr, fg, fb);
    }

    @Override
    public void drawQuads(float[] data, int nQuads) {
        if (nQuads <= 0) return;
        qfb.clear();
        qfb.put(data, 0, nQuads * 36);
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, 0);
        qfb.position(0);
        GLES20.glVertexAttribPointer(A_POS, 3, GLES20.GL_FLOAT, false, 36, qfb);
        qfb.position(3);
        GLES20.glVertexAttribPointer(A_UV, 2, GLES20.GL_FLOAT, false, 36, qfb);
        qfb.position(5);
        GLES20.glVertexAttribPointer(A_COL, 4, GLES20.GL_FLOAT, false, 36, qfb);
        GLES20.glBindBuffer(GLES20.GL_ELEMENT_ARRAY_BUFFER, quadIbo);
        GLES20.glDrawElements(GLES20.GL_TRIANGLES, nQuads * 6, GLES20.GL_UNSIGNED_SHORT, 0);
        bound = null;
    }
}
