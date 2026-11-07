package com.monolito.correlula;

/**
 * Camada grafica minima. No Android e implementada com OpenGL ES 2.0 (GLBackend);
 * nos testes de desktop por um rasterizador em software.
 */
interface Gfx {
    /** Malha estatica: vertices x,y,z,r,g,b (6 floats) + indices short. */
    final class Mesh {
        int vbo, ibo, count;      // uso do backend GL
        float[] v; short[] idx;   // uso do backend de software
    }

    Mesh createMesh(float[] v, int nFloats, short[] idx, int nIdx);
    void deleteMesh(Mesh m);

    /** Pixels ARGB nao pre-multiplicados. */
    int createTexture(int[] argb, int w, int h);

    void viewport(int w, int h);
    void clear(float r, float g, float b);
    void state(boolean depthTest, boolean depthWrite, boolean blend, boolean cull);

    /** Inicia passe de malhas coloridas. fogStart >= fogEnd desliga a neblina. */
    void colorPass(float[] vp, float fogStart, float fogEnd, float fr, float fg, float fb);
    void drawMesh(Mesh m, float[] model, float r, float g, float b, float a);

    /** Inicia passe de quads texturizados. */
    void texPass(float[] vp, int tex, float fogStart, float fogEnd, float fr, float fg, float fb);
    /** 4 vertices por quad, 9 floats cada: x,y,z,u,v,r,g,b,a. */
    void drawQuads(float[] data, int nQuads);
}
